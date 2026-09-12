use std::ffi::{CStr, c_char};
use std::mem::{size_of, size_of_val};
use std::ptr::NonNull;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use crate::pack::ConnectomePack;
use crate::parameters::ModelParameters;
use crate::stimulus::EventSchedule;

#[derive(Clone, Debug, PartialEq)]
pub struct CudaStep {
    pub spikes: Vec<u8>,
    pub voltage_mv: Vec<f32>,
    pub conductance_mv: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct CudaRun {
    pub elapsed: Duration,
    pub spike_counts: Vec<u32>,
    pub voltage_mv: Vec<f32>,
    pub conductance_mv: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct CudaWindow {
    pub elapsed: Duration,
    pub spike_count_deltas: Vec<u32>,
}

pub type CudaWindowResult = CudaWindow;
type StateCopy = (Vec<u8>, Vec<u32>, Vec<f32>, Vec<f32>);

#[repr(C)]
struct RawCudaEngine {
    _private: [u8; 0],
}

#[repr(C)]
struct CudaMetrics {
    total_spikes: u64,
    active_neurons: u32,
    voltage_sum: f64,
}

unsafe extern "C" {
    fn flybrain_cuda_last_error() -> *const c_char;
    fn flybrain_cuda_create(
        output: *mut *mut RawCudaEngine,
        row_ptr: *const u32,
        row_ptr_count: usize,
        destinations: *const u32,
        signed_counts: *const i16,
        edge_count: usize,
        silenced_sources: *const u8,
        initial_voltage: *const f32,
        initial_conductance: *const f32,
        refractory_lengths: *const i32,
        external_targets: *const u32,
        external_target_count: u32,
        chunked_propagation: u8,
        neuron_count: u32,
        delay_steps: u32,
        resting_mv: f32,
        reset_mv: f32,
        threshold_mv: f32,
        membrane_decay: f32,
        synapse_decay: f32,
        coupling: f32,
        synapse_weight_mv: f32,
        external_weight_mv: f32,
    ) -> i32;
    fn flybrain_cuda_destroy(engine: *mut RawCudaEngine);
    fn flybrain_cuda_device_name(engine: *mut RawCudaEngine) -> *const c_char;
    fn flybrain_cuda_run_dense(
        engine: *mut RawCudaEngine,
        targets: *const u32,
        target_count: u32,
        counts: *const u8,
        steps: u32,
        chunk_steps: u32,
    ) -> i32;
    fn flybrain_cuda_run_sparse_probed(
        engine: *mut RawCudaEngine,
        step_offsets: *const u32,
        lanes: *const u32,
        counts: *const u8,
        event_count: u32,
        steps: u32,
        probes: *const u32,
        probe_count: u32,
        refresh_probes: u8,
        before: *mut u32,
        after: *mut u32,
    ) -> i32;
    fn flybrain_cuda_read_metrics(
        engine: *mut RawCudaEngine,
        total_spikes: *mut u64,
        active_neurons: *mut u32,
        voltage_sum: *mut f64,
    ) -> i32;
    fn flybrain_cuda_copy_state(
        engine: *mut RawCudaEngine,
        spikes: *mut u8,
        spike_counts: *mut u32,
        voltage: *mut f32,
        conductance: *mut f32,
    ) -> i32;
}

pub struct CudaEngine {
    raw: NonNull<RawCudaEngine>,
    device_name: String,
    propagation_mode: &'static str,
    neuron_count: usize,
    external_targets: Vec<u32>,
    allocated_bytes: usize,
    probe_capacity: usize,
    sparse_event_capacity: usize,
    cached_probe_indices: Vec<u32>,
    cached_probe_counts: Vec<u32>,
}

impl CudaEngine {
    pub fn new(
        connectome: &ConnectomePack,
        parameters: ModelParameters,
        initial_voltage_mv: Option<&[f64]>,
        initial_conductance_mv: Option<&[f64]>,
        zero_refractory: &[u32],
        silenced_sources: &[u32],
    ) -> Result<Self> {
        let parameters = parameters.validate()?;
        let neuron_count = connectome.neuron_count();
        if neuron_count == 0 {
            bail!("CUDA engine requires at least one neuron");
        }
        let neuron_count_u32 = u32::try_from(neuron_count).context("neuron count overflow")?;
        let delay_steps = u32::try_from(parameters.delay_steps()).context("delay overflow")?;
        let initial_voltage = f32_state(
            initial_voltage_mv,
            neuron_count,
            parameters.resting_mv,
            "initial voltage",
        )?;
        let initial_conductance = f32_state(
            initial_conductance_mv,
            neuron_count,
            0.0,
            "initial conductance",
        )?;
        let mut refractory_lengths = vec![parameters.refractory_steps(); neuron_count];
        for &neuron in zero_refractory {
            refractory_lengths[checked_index(neuron, neuron_count, "zero-refractory neuron")?] = 0;
        }
        let mut silenced = vec![0_u8; neuron_count];
        for &source in silenced_sources {
            silenced[checked_index(source, neuron_count, "silenced source")?] = 1;
        }

        let mut raw = std::ptr::null_mut();
        let chunked_propagation = match std::env::var("FLYBRAIN_CUDA_PROPAGATION") {
            Ok(value) if value == "chunked-256" => true,
            Ok(value) if value == "source-serial" => false,
            Ok(value) => bail!(
                "unsupported FLYBRAIN_CUDA_PROPAGATION={value:?}; expected source-serial or chunked-256"
            ),
            Err(std::env::VarError::NotPresent) => true,
            Err(error) => return Err(error).context("reading FLYBRAIN_CUDA_PROPAGATION"),
        };
        let propagation_task_count = if chunked_propagation {
            connectome
                .row_ptr
                .windows(2)
                .map(|row| (row[1] - row[0]).div_ceil(256) as usize)
                .sum()
        } else {
            0
        };
        let status = unsafe {
            flybrain_cuda_create(
                &mut raw,
                connectome.row_ptr.as_ptr(),
                connectome.row_ptr.len(),
                connectome.destinations.as_ptr(),
                connectome.signed_counts.as_ptr(),
                connectome.edge_count(),
                silenced.as_ptr(),
                initial_voltage.as_ptr(),
                initial_conductance.as_ptr(),
                refractory_lengths.as_ptr(),
                zero_refractory.as_ptr(),
                u32::try_from(zero_refractory.len()).context("external target count overflow")?,
                u8::from(chunked_propagation),
                neuron_count_u32,
                delay_steps,
                parameters.resting_mv as f32,
                parameters.reset_mv as f32,
                parameters.threshold_mv as f32,
                parameters.membrane_decay() as f32,
                parameters.synapse_decay() as f32,
                parameters.coupling() as f32,
                parameters.synapse_weight_mv as f32,
                parameters.external_weight_mv as f32,
            )
        };
        check_cuda(status)?;
        let raw = NonNull::new(raw).context("CUDA constructor returned a null engine")?;
        let device_name = unsafe {
            CStr::from_ptr(flybrain_cuda_device_name(raw.as_ptr()))
                .to_string_lossy()
                .into_owned()
        };
        let ring_size = parameters.delay_steps().max(1);
        let allocated_bytes = size_of_val(connectome.row_ptr.as_slice())
            + size_of_val(connectome.destinations.as_slice())
            + size_of_val(connectome.signed_counts.as_slice())
            + size_of_val(silenced.as_slice())
            + size_of_val(initial_voltage.as_slice())
            + size_of_val(initial_conductance.as_slice())
            + neuron_count * size_of::<i32>()
            + size_of_val(refractory_lengths.as_slice())
            + neuron_count
            + neuron_count * size_of::<u32>()
            + neuron_count * ring_size
            + neuron_count * size_of::<i32>();

        Ok(Self {
            raw,
            device_name,
            propagation_mode: if chunked_propagation {
                "chunked-256"
            } else {
                "source-serial"
            },
            neuron_count,
            external_targets: zero_refractory.to_vec(),
            allocated_bytes: allocated_bytes
                + size_of::<CudaMetrics>()
                + size_of_val(zero_refractory)
                + propagation_task_count * 3 * size_of::<u32>(),
            probe_capacity: 0,
            sparse_event_capacity: 0,
            cached_probe_indices: Vec::new(),
            cached_probe_counts: Vec::new(),
        })
    }

    pub fn run_schedule(
        &mut self,
        schedule: &EventSchedule,
        chunk_steps: usize,
    ) -> Result<CudaRun> {
        self.validate_schedule(schedule)?;
        self.invalidate_probe_cache();
        let steps = u32::try_from(schedule.steps()).context("schedule step count overflow")?;
        let target_count =
            u32::try_from(schedule.targets().len()).context("target count overflow")?;
        let chunk_steps = u32::try_from(chunk_steps.max(1)).context("chunk size overflow")?;
        let started = Instant::now();
        check_cuda(unsafe {
            flybrain_cuda_run_dense(
                self.raw.as_ptr(),
                schedule.targets().as_ptr(),
                target_count,
                schedule.counts().as_ptr(),
                steps,
                chunk_steps,
            )
        })?;
        let elapsed = started.elapsed();
        let (_, spike_counts, voltage_mv, conductance_mv) = self.copy_state()?;
        Ok(CudaRun {
            elapsed,
            spike_counts,
            voltage_mv,
            conductance_mv,
        })
    }

    pub fn run_recorded(&mut self, schedule: &EventSchedule) -> Result<Vec<CudaStep>> {
        self.validate_schedule(schedule)?;
        self.invalidate_probe_cache();
        let target_count =
            u32::try_from(schedule.targets().len()).context("target count overflow")?;
        let mut trace = Vec::with_capacity(schedule.steps());
        for step in 0..schedule.steps() {
            let start = step * schedule.targets().len();
            let end = start + schedule.targets().len();
            check_cuda(unsafe {
                flybrain_cuda_run_dense(
                    self.raw.as_ptr(),
                    schedule.targets().as_ptr(),
                    target_count,
                    schedule.counts()[start..end].as_ptr(),
                    1,
                    1,
                )
            })?;
            let (spikes, _, voltage_mv, conductance_mv) = self.copy_state()?;
            trace.push(CudaStep {
                spikes,
                voltage_mv,
                conductance_mv,
            });
        }
        Ok(trace)
    }

    pub fn run_window(
        &mut self,
        schedule: &EventSchedule,
        probe_neurons: &[u32],
    ) -> Result<CudaWindow> {
        self.validate_schedule(schedule)?;
        validate_indices(probe_neurons, self.neuron_count, "probe neuron")?;
        self.invalidate_probe_cache();
        let before_counts = self.spike_counts()?;
        let before = probe_counts(&before_counts, probe_neurons);
        let steps = u32::try_from(schedule.steps()).context("schedule step count overflow")?;
        let target_count =
            u32::try_from(schedule.targets().len()).context("target count overflow")?;
        let started = Instant::now();
        check_cuda(unsafe {
            flybrain_cuda_run_dense(
                self.raw.as_ptr(),
                schedule.targets().as_ptr(),
                target_count,
                schedule.counts().as_ptr(),
                steps,
                steps.max(1),
            )
        })?;
        let elapsed = started.elapsed();
        let after_counts = self.spike_counts()?;
        window_result(before, probe_counts(&after_counts, probe_neurons), elapsed)
    }

    pub fn run_window_sparse(
        &mut self,
        steps: usize,
        step_offsets: &[u32],
        lanes: &[u32],
        counts: &[u8],
        probe_neurons: &[u32],
    ) -> Result<CudaWindow> {
        self.validate_sparse_window(steps, step_offsets, lanes, counts)?;
        validate_indices(probe_neurons, self.neuron_count, "probe neuron")?;
        let steps_u32 = u32::try_from(steps).context("sparse step count overflow")?;
        let event_count = u32::try_from(lanes.len()).context("event count overflow")?;
        let probe_count = u32::try_from(probe_neurons.len()).context("probe count overflow")?;
        let refresh_probes = self.cached_probe_indices.as_slice() != probe_neurons;
        let mut before = if refresh_probes {
            vec![0; probe_neurons.len()]
        } else {
            self.cached_probe_counts.clone()
        };
        let mut after = vec![0; probe_neurons.len()];
        let started = Instant::now();
        check_cuda(unsafe {
            flybrain_cuda_run_sparse_probed(
                self.raw.as_ptr(),
                step_offsets.as_ptr(),
                lanes.as_ptr(),
                counts.as_ptr(),
                event_count,
                steps_u32,
                probe_neurons.as_ptr(),
                probe_count,
                u8::from(refresh_probes),
                if refresh_probes {
                    before.as_mut_ptr()
                } else {
                    std::ptr::null_mut()
                },
                after.as_mut_ptr(),
            )
        })?;
        let elapsed = started.elapsed();
        if probe_neurons.len() > self.probe_capacity {
            self.allocated_bytes +=
                (probe_neurons.len() - self.probe_capacity) * 3 * size_of::<u32>();
            self.probe_capacity = probe_neurons.len();
        }
        if lanes.len() > self.sparse_event_capacity {
            self.allocated_bytes +=
                (lanes.len() - self.sparse_event_capacity) * (size_of::<u32>() + size_of::<u8>());
            self.sparse_event_capacity = lanes.len();
        }
        if refresh_probes {
            self.cached_probe_indices = probe_neurons.to_vec();
        }
        self.cached_probe_counts.clone_from(&after);
        window_result(before, after, elapsed)
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    pub fn propagation_mode(&self) -> &str {
        self.propagation_mode
    }

    pub fn allocated_bytes(&self) -> usize {
        self.allocated_bytes
    }

    pub fn total_spike_count(&self) -> Result<u64> {
        Ok(self.metrics()?.0)
    }

    pub fn spiking_neuron_count(&self) -> Result<usize> {
        Ok(self.metrics()?.1)
    }

    pub fn population_counts(&self) -> Result<(u64, usize)> {
        let (total, active, _) = self.metrics()?;
        Ok((total, active))
    }

    pub fn mean_voltage_deviation_mv(&self, resting_mv: f64) -> Result<f64> {
        let (_, _, voltage_sum) = self.metrics()?;
        Ok(voltage_sum / self.neuron_count as f64 - resting_mv)
    }

    fn validate_schedule(&self, schedule: &EventSchedule) -> Result<()> {
        validate_indices(schedule.targets(), self.neuron_count, "stimulus target")?;
        let event_slots = schedule
            .steps()
            .checked_mul(schedule.targets().len())
            .context("stimulus dimensions overflow")?;
        u32::try_from(event_slots).context("dense stimulus offset exceeds CUDA uint range")?;
        Ok(())
    }

    fn validate_sparse_window(
        &self,
        steps: usize,
        step_offsets: &[u32],
        lanes: &[u32],
        counts: &[u8],
    ) -> Result<()> {
        if lanes.len() > u32::MAX as usize
            || step_offsets.len() != steps.checked_add(1).context("step count overflow")?
            || step_offsets.first().copied() != Some(0)
            || step_offsets.last().copied() != Some(lanes.len() as u32)
            || lanes.len() != counts.len()
            || counts.contains(&0)
        {
            bail!("sparse stimulus window dimensions or counts are invalid");
        }
        for offsets in step_offsets.windows(2) {
            if offsets[0] > offsets[1] || offsets[1] as usize > lanes.len() {
                bail!("sparse stimulus step offsets are invalid");
            }
            let step_lanes = &lanes[offsets[0] as usize..offsets[1] as usize];
            if step_lanes
                .iter()
                .any(|&lane| lane as usize >= self.external_targets.len())
                || step_lanes.windows(2).any(|pair| pair[0] >= pair[1])
            {
                bail!("sparse stimulus lanes must be in range and strictly increasing per step");
            }
        }
        Ok(())
    }

    fn spike_counts(&self) -> Result<Vec<u32>> {
        let (_, counts, _, _) = self.copy_state()?;
        Ok(counts)
    }

    fn copy_state(&self) -> Result<StateCopy> {
        let mut spikes = vec![0; self.neuron_count];
        let mut spike_counts = vec![0; self.neuron_count];
        let mut voltage = vec![0.0; self.neuron_count];
        let mut conductance = vec![0.0; self.neuron_count];
        check_cuda(unsafe {
            flybrain_cuda_copy_state(
                self.raw.as_ptr(),
                spikes.as_mut_ptr(),
                spike_counts.as_mut_ptr(),
                voltage.as_mut_ptr(),
                conductance.as_mut_ptr(),
            )
        })?;
        Ok((spikes, spike_counts, voltage, conductance))
    }

    fn metrics(&self) -> Result<(u64, usize, f64)> {
        let mut total_spikes = 0;
        let mut active_neurons = 0;
        let mut voltage_sum = 0.0;
        check_cuda(unsafe {
            flybrain_cuda_read_metrics(
                self.raw.as_ptr(),
                &mut total_spikes,
                &mut active_neurons,
                &mut voltage_sum,
            )
        })?;
        Ok((total_spikes, active_neurons as usize, voltage_sum))
    }

    fn invalidate_probe_cache(&mut self) {
        self.cached_probe_indices.clear();
        self.cached_probe_counts.clear();
    }
}

fn window_result(before: Vec<u32>, after: Vec<u32>, elapsed: Duration) -> Result<CudaWindow> {
    let spike_count_deltas = before
        .into_iter()
        .zip(after)
        .map(|(before, after)| {
            after
                .checked_sub(before)
                .context("CUDA probe spike count moved backwards")
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(CudaWindow {
        elapsed,
        spike_count_deltas,
    })
}

fn probe_counts(counts: &[u32], probes: &[u32]) -> Vec<u32> {
    probes.iter().map(|&probe| counts[probe as usize]).collect()
}

impl Drop for CudaEngine {
    fn drop(&mut self) {
        unsafe { flybrain_cuda_destroy(self.raw.as_ptr()) };
    }
}

fn check_cuda(status: i32) -> Result<()> {
    if status == 0 {
        return Ok(());
    }
    let message = unsafe {
        let pointer = flybrain_cuda_last_error();
        if pointer.is_null() {
            "unknown CUDA error".to_owned()
        } else {
            CStr::from_ptr(pointer).to_string_lossy().into_owned()
        }
    };
    bail!("{message}")
}

fn validate_indices(indices: &[u32], size: usize, name: &str) -> Result<()> {
    for &index in indices {
        checked_index(index, size, name)?;
    }
    Ok(())
}

fn checked_index(index: u32, size: usize, name: &str) -> Result<usize> {
    let index = index as usize;
    if index >= size {
        bail!("{name} {index} is outside [0, {size})");
    }
    Ok(index)
}

fn f32_state(values: Option<&[f64]>, size: usize, default: f64, name: &str) -> Result<Vec<f32>> {
    let values = values.map_or_else(|| vec![default; size], <[f64]>::to_vec);
    if values.len() != size {
        bail!("{name} must have one value per neuron");
    }
    if values.iter().any(|value| !value.is_finite()) {
        bail!("{name} must contain finite values");
    }
    Ok(values.into_iter().map(|value| value as f32).collect())
}

#[cfg(test)]
#[path = "cuda_engine_tests.rs"]
mod tests;
