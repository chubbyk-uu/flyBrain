//! Native viewer transport. The simulation and its GPU context are created and
//! destroyed on the worker; rendering only touches an independent MjData copy.
use super::*;
use flybrain_engine::live_viewer::LiveInput;
use flybrain_engine::retina::RetinaSummary;
use mujoco_rs::prelude::{MjData, MjModel};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;

pub struct Frame {
    pub data: MjData<Box<MjModel>>,
    pub snapshot: SimulationSnapshot,
    pub paused: bool,
    pub epoch: u64,
    pub sequence: u64,
    pub realtime_factor: f64,
    pub neurons: usize,
    pub sensory_neurons: usize,
    pub nearest_resource: String,
    pub tasted_resource: String,
    pub nearest_obstacle: String,
    pub field_samples: Vec<(f64, f64, f64)>,
}

pub enum Command {
    Input(LiveInput),
    Telemetry(bool),
}

type VisionMailbox = Arc<Mutex<Option<(u64, [RetinaSummary; 2])>>>;

pub struct Worker {
    pub frame: Arc<Mutex<Frame>>,
    pub vision: VisionMailbox,
    pub commands: mpsc::SyncSender<Command>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<()>>>,
}

impl Worker {
    pub fn start(options: ViewOptions) -> Result<Self> {
        let (commands, receiver) = mpsc::sync_channel(64);
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let vision = Arc::new(Mutex::new(None));
        let worker_vision = vision.clone();
        let thread = std::thread::Builder::new()
            .name("flybrain-simulation".into())
            .spawn(move || {
                let parameters = options
                    .parameters
                    .as_deref()
                    .map(SimulationParameters::load)
                    .transpose()?
                    .unwrap_or_default();
                let mut simulation = SimulationStepper::new_with_parameters(
                    &options.assets,
                    options.with_brain.then_some(options.pack.as_path()),
                    options.control_hz,
                    options.settle_seconds,
                    parameters,
                )?;
                simulation.place_food_ahead(options.start_food_distance)?;
                simulation.set_brain_telemetry_enabled(options.with_brain)?;
                eprintln!(
                    "Simulation worker ready: {} neurons on {}",
                    simulation.brain_neuron_count(),
                    simulation.brain_device_name().unwrap_or("disabled")
                );
                let frame = Arc::new(Mutex::new(Frame {
                    data: simulation.world().data().clone(),
                    snapshot: simulation.snapshot(),
                    paused: false,
                    epoch: 0,
                    sequence: 0,
                    realtime_factor: 0.0,
                    neurons: simulation.brain_neuron_count(),
                    sensory_neurons: simulation.brain_sensory_neuron_count(),
                    nearest_resource: String::new(),
                    tasted_resource: String::new(),
                    nearest_obstacle: String::new(),
                    field_samples: Vec::new(),
                }));
                ready_tx
                    .send(frame.clone())
                    .map_err(|_| anyhow::anyhow!("viewer closed during startup"))?;
                let mut paused = false;
                let mut epoch = 0;
                let mut anchor = (Instant::now(), simulation.world().time());
                let mut stats = anchor;
                let mut realtime_factor = 0.0;
                let mut last_publish = Instant::now() - Duration::from_secs(1);
                let mut field_sequence = 0;
                let mut field_samples = Vec::new();
                let period = simulation.control_period().as_secs_f64();
                while !worker_stop.load(Ordering::Relaxed) {
                    let mut force_publish = false;
                    for command in receiver.try_iter() {
                        match command {
                            Command::Telemetry(enabled) => {
                                simulation.set_brain_telemetry_enabled(enabled)?
                            }
                            Command::Input(input) => {
                                if input.toggle_pause {
                                    paused = !paused;
                                    anchor = (Instant::now(), simulation.world().time());
                                    stats = anchor;
                                }
                                if input.reset {
                                    simulation.reset()?;
                                    epoch += 1;
                                    field_samples.clear();
                                    field_sequence = 0;
                                    *worker_vision.lock().unwrap() = None;
                                    anchor = (Instant::now(), simulation.world().time());
                                    stats = anchor;
                                    realtime_factor = 0.0;
                                }
                                if input.toggle_food {
                                    simulation.toggle_food()?;
                                }
                                if input.toggle_flight {
                                    simulation.toggle_flight();
                                }
                                if input.request_grooming {
                                    simulation.request_grooming();
                                }
                                if input.place_food_at_mouth {
                                    simulation.drop_food_below_fly()?;
                                }
                                if input.food_motion != [0.0; 2] {
                                    simulation.move_food([
                                        input.food_motion[0],
                                        input.food_motion[1],
                                        0.0,
                                    ])?;
                                }
                                force_publish = true;
                            }
                        }
                    }
                    if let Some((vision_epoch, summaries)) = worker_vision.lock().unwrap().take()
                        && vision_epoch == epoch
                    {
                        simulation.set_retina_summaries(summaries)?;
                    }
                    let due = simulation.world().time() + period * 0.5
                        < anchor.1 + anchor.0.elapsed().as_secs_f64() * options.speed;
                    if !paused && due {
                        let snapshot = simulation.step_window()?;
                        if snapshot.brain_field_sample_sequence != field_sequence {
                            field_sequence = snapshot.brain_field_sample_sequence;
                            field_samples.push((
                                snapshot.time_seconds,
                                snapshot.brain_field_potential_mv,
                                snapshot.brain_field_dominant_frequency_hz,
                            ));
                            // Bound display history even if the renderer stalls indefinitely.
                            if field_samples.len() > 1000 {
                                field_samples.remove(0);
                            }
                        }
                    }
                    if stats.0.elapsed() >= Duration::from_millis(500) {
                        realtime_factor =
                            (simulation.world().time() - stats.1) / stats.0.elapsed().as_secs_f64();
                        stats = (Instant::now(), simulation.world().time());
                    }
                    let finished = options
                        .max_seconds
                        .is_some_and(|limit| simulation.world().time() >= limit);
                    if force_publish
                        || finished
                        || last_publish.elapsed()
                            >= Duration::from_secs_f64(1.0 / f64::from(options.fps))
                    {
                        let mut frame = frame.lock().unwrap();
                        simulation.world().data().copy_to(&mut frame.data)?;
                        let snapshot = simulation.snapshot();
                        frame.snapshot = snapshot;
                        frame.paused = paused;
                        if frame.epoch != epoch {
                            frame.field_samples.clear();
                        }
                        frame.epoch = epoch;
                        frame.sequence += 1;
                        frame.realtime_factor = realtime_factor;
                        frame.nearest_resource =
                            simulation.resource_label(snapshot.nearest_resource).into();
                        frame.tasted_resource =
                            simulation.resource_label(snapshot.tasted_resource).into();
                        frame.nearest_obstacle = simulation
                            .obstacle_label(snapshot.flight_nearest_obstacle_geom_id)
                            .into();
                        frame.field_samples.append(&mut field_samples);
                        let excess = frame.field_samples.len().saturating_sub(1000);
                        frame.field_samples.drain(..excess);
                        last_publish = Instant::now();
                    }
                    if finished {
                        break;
                    }
                    if paused || !due {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }
                Ok(())
            })?;
        match ready_rx.recv() {
            Ok(frame) => Ok(Self {
                frame,
                vision,
                commands,
                stop,
                thread: Some(thread),
            }),
            Err(_) => {
                thread
                    .join()
                    .map_err(|_| anyhow::anyhow!("simulation worker panicked during startup"))??;
                bail!("simulation worker exited before startup")
            }
        }
    }

    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }

    pub fn finish(&mut self) -> Result<()> {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| anyhow::anyhow!("simulation worker panicked"))??;
        }
        Ok(())
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        if let Err(error) = self.finish() {
            eprintln!("Simulation worker: {error:#}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> ViewOptions {
        ViewOptions {
            assets: DEFAULT_ASSETS_DIR.into(),
            pack: "outputs/packs/male_cns_v1".into(),
            width: 640,
            height: 480,
            fps: 60,
            control_hz: 500.0,
            settle_seconds: 0.5,
            speed: 1.0,
            start_food_distance: 40.0,
            camera: "chase".into(),
            max_seconds: None,
            with_brain: false,
            parameters: None,
        }
    }

    fn wait_until(mut condition: impl FnMut() -> bool) {
        let start = Instant::now();
        while !condition() {
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "worker did not respond"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn worker_preserves_direct_world_state_without_renderer() {
        compare_direct_and_worker(false);
    }

    #[test]
    #[ignore = "requires the full MaleCNS pack and native GPU backend"]
    fn worker_preserves_full_cns_state_with_fixed_visual_input() {
        compare_direct_and_worker(true);
    }

    fn compare_direct_and_worker(with_brain: bool) {
        let mut opts = options();
        opts.with_brain = with_brain;
        opts.speed = 1000.0;
        opts.max_seconds = Some(0.019);
        let mut direct = SimulationStepper::new_with_parameters(
            &opts.assets,
            with_brain.then_some(opts.pack.as_path()),
            opts.control_hz,
            opts.settle_seconds,
            SimulationParameters::default(),
        )
        .unwrap();
        direct.place_food_ahead(40.0).unwrap();
        direct.set_brain_telemetry_enabled(with_brain).unwrap();
        let mut worker = Worker::start(opts).unwrap();
        wait_until(|| worker.is_finished());
        worker.finish().unwrap();
        for _ in 0..10 {
            direct.step_window().unwrap();
        }
        let frame = worker.frame.lock().unwrap();
        assert_eq!(frame.data.time(), direct.world().time());
        assert_eq!(frame.data.qpos(), direct.world().data().qpos());
        assert_eq!(frame.data.qvel(), direct.world().data().qvel());
        assert_eq!(frame.data.ctrl(), direct.world().data().ctrl());
        assert_eq!(frame.data.sensordata(), direct.world().data().sensordata());
        assert_eq!(
            frame.snapshot.population_spike_delta,
            direct.snapshot().population_spike_delta
        );
        assert_eq!(
            frame.snapshot.cumulative_spiking_neuron_count,
            direct.snapshot().cumulative_spiking_neuron_count
        );
        assert_eq!(
            frame.snapshot.filtered_population_rate_hz,
            direct.snapshot().filtered_population_rate_hz
        );
        assert_eq!(
            frame.snapshot.mn9_spike_delta,
            direct.snapshot().mn9_spike_delta
        );
    }

    #[test]
    fn pause_reset_rejects_old_vision_and_exits_without_renderer() {
        let mut worker = Worker::start(options()).unwrap();
        worker
            .commands
            .send(Command::Input(LiveInput {
                toggle_pause: true,
                ..LiveInput::default()
            }))
            .unwrap();
        wait_until(|| worker.frame.lock().unwrap().paused);
        let time = worker.frame.lock().unwrap().snapshot.time_seconds;
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(worker.frame.lock().unwrap().snapshot.time_seconds, time);
        worker
            .commands
            .send(Command::Input(LiveInput {
                reset: true,
                ..LiveInput::default()
            }))
            .unwrap();
        wait_until(|| worker.frame.lock().unwrap().epoch == 1);
        *worker.vision.lock().unwrap() = Some((
            0,
            [RetinaSummary {
                mean_intensity: 1.0,
                spatial_contrast: 0.5,
            }; 2],
        ));
        wait_until(|| worker.vision.lock().unwrap().is_none());
        {
            let frame = worker.frame.lock().unwrap();
            assert_eq!(frame.snapshot.time_seconds, 0.0);
            assert_eq!(frame.snapshot.visual_left, 0.0);
            assert!(frame.paused);
        }
        worker
            .commands
            .send(Command::Input(LiveInput {
                toggle_pause: true,
                ..LiveInput::default()
            }))
            .unwrap();
        wait_until(|| worker.frame.lock().unwrap().snapshot.time_seconds > 0.0);
        worker.finish().unwrap();
    }

    #[test]
    fn worker_propagates_startup_failure() {
        let mut opts = options();
        opts.assets = "missing-viewer-test-assets".into();
        assert!(Worker::start(opts).is_err());
    }
}
