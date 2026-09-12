use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::Parser;
use flybrain_engine::cns_pathway::{CnsPathway, PathwayControl, ResolvedPathway};
use flybrain_engine::cuda_engine::{CudaEngine, CudaRun};
use flybrain_engine::neural_io::{NeuralIoArtifact, NeuralIoResolution};
use flybrain_engine::pack::ConnectomePack;
use flybrain_engine::parameters::ModelParameters;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Parser)]
#[command(about = "Validate the standalone CUDA engine on the complete MaleCNS pack")]
struct Options {
    #[arg(long, default_value = "experiments/male_cns_cuda_validation_v1.json")]
    protocol: PathBuf,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Protocol {
    schema: String,
    schema_version: u32,
    pack_path: PathBuf,
    neural_io_path: PathBuf,
    neural_io_sha256: String,
    pathway_path: PathBuf,
    pathway_sha256: String,
    reference_summary_path: PathBuf,
    reference_summary_sha256: String,
    steps: usize,
    rate_hz: f64,
    seeds: Vec<u64>,
    controls: Vec<String>,
    repeat_runs: usize,
    chunk_steps: usize,
    chunk_invariance: ChunkInvariance,
    short_reference: ShortReference,
    reference_cases: Vec<ReferenceCase>,
    acceptance: Acceptance,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChunkInvariance {
    seed: u64,
    control: String,
    alternate_chunk_steps: usize,
    require_exact_spike_counts: bool,
    require_exact_final_f32_state: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ShortReference {
    steps: usize,
    seed: u64,
    control: String,
    path: PathBuf,
    sha256: String,
    require_exact_spike_count_hash: bool,
    final_f32_state_hash_is_diagnostic: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReferenceCase {
    seed: u64,
    control: String,
    path: PathBuf,
    sha256: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Acceptance {
    cuda_repeats_exact: bool,
    population_totals_match_reference_exactly: bool,
    input_relay_and_each_motor_pool_match_reference_exactly: bool,
    software_controls_pass: bool,
    expected_pathway_response_reproduced_all_seeds: bool,
    long_run_cpu_f64_per_neuron_equality_required: bool,
    body_behavior_evaluated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Counts {
    per_neuron: Vec<u32>,
    total: u64,
    active: usize,
}

#[derive(Clone, Debug)]
struct ComputedCase {
    seed: u64,
    control: String,
    input: Counts,
    relay: Counts,
    readouts: BTreeMap<String, Counts>,
    population: Counts,
    hashes: Hashes,
    report: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Hashes {
    spike_counts: String,
    voltage: String,
    conductance: String,
    state: String,
}

fn main() -> Result<()> {
    let options = Options::parse();
    let protocol_bytes = read_verified_json_bytes(&options.protocol, None)?;
    let protocol_sha256 = sha256_bytes(&protocol_bytes);
    let protocol: Protocol = serde_json::from_slice(&protocol_bytes)
        .with_context(|| format!("parsing {}", options.protocol.display()))?;
    validate_protocol(&protocol)?;

    let _ = read_verified_json_bytes(&protocol.neural_io_path, Some(&protocol.neural_io_sha256))?;
    let _ = read_verified_json_bytes(&protocol.pathway_path, Some(&protocol.pathway_sha256))?;
    let reference_summary_bytes = read_verified_json_bytes(
        &protocol.reference_summary_path,
        Some(&protocol.reference_summary_sha256),
    )?;
    let reference_summary: Value = serde_json::from_slice(&reference_summary_bytes)?;

    let pack = ConnectomePack::open(&protocol.pack_path)?;
    let neural_io = NeuralIoArtifact::load(&protocol.neural_io_path)?.resolve(&pack)?;
    let pathway = CnsPathway::load(&protocol.pathway_path)?;
    let resolved = pathway.resolve(&pack)?;
    let parameters = ModelParameters::default().validate()?;

    let mut cases = Vec::with_capacity(protocol.reference_cases.len());
    let mut all_repeats_exact = true;
    let mut all_population_matches = true;
    let mut all_groups_match = true;
    let mut first_reference_difference = None;
    for reference_case in &protocol.reference_cases {
        let control = parse_control(&reference_case.control)?;
        let reference_bytes =
            read_verified_json_bytes(&reference_case.path, Some(&reference_case.sha256))?;
        let reference: Value = serde_json::from_slice(&reference_bytes)?;
        let computed = run_case(
            &pack,
            &neural_io,
            &resolved,
            parameters,
            reference_case.seed,
            control,
            protocol.steps,
            protocol.rate_hz,
            protocol.chunk_steps,
        )?;
        let comparison = compare_reference(&computed, &reference)?;
        all_population_matches &= comparison.population_match;
        all_groups_match &= comparison.groups_match;
        if first_reference_difference.is_none() {
            first_reference_difference = comparison.first_difference;
        }

        let mut repeat_hashes = vec![hashes_json(&computed.hashes)];
        for _ in 1..protocol.repeat_runs {
            let repeat = run_case(
                &pack,
                &neural_io,
                &resolved,
                parameters,
                reference_case.seed,
                control,
                protocol.steps,
                protocol.rate_hz,
                protocol.chunk_steps,
            )?;
            all_repeats_exact &= repeat.hashes == computed.hashes;
            repeat_hashes.push(hashes_json(&repeat.hashes));
        }
        let mut report = computed.report.clone();
        report["reference"] = json!({
            "path": reference_case.path,
            "sha256": reference_case.sha256,
            "population_match": comparison.population_match,
            "input_relay_motor_groups_match": comparison.groups_match,
            "spike_count_hash_match": comparison.spike_hash_match,
            "first_difference": comparison.first_difference_for_report,
        });
        report["repeat_hashes"] = Value::Array(repeat_hashes);
        cases.push(ComputedCase { report, ..computed });
    }

    let chunk_control = parse_control(&protocol.chunk_invariance.control)?;
    let primary = find_case(
        &cases,
        protocol.chunk_invariance.seed,
        &protocol.chunk_invariance.control,
    )?;
    let alternate = run_case(
        &pack,
        &neural_io,
        &resolved,
        parameters,
        protocol.chunk_invariance.seed,
        chunk_control,
        protocol.steps,
        protocol.rate_hz,
        protocol.chunk_invariance.alternate_chunk_steps,
    )?;
    let chunk_spikes_exact = primary.hashes.spike_counts == alternate.hashes.spike_counts;
    let chunk_state_exact = primary.hashes.state == alternate.hashes.state;

    let short_control = parse_control(&protocol.short_reference.control)?;
    let short_reference_bytes = read_verified_json_bytes(
        &protocol.short_reference.path,
        Some(&protocol.short_reference.sha256),
    )?;
    let short_reference: Value = serde_json::from_slice(&short_reference_bytes)?;
    let short = run_case(
        &pack,
        &neural_io,
        &resolved,
        parameters,
        protocol.short_reference.seed,
        short_control,
        protocol.short_reference.steps,
        protocol.rate_hz,
        protocol.chunk_steps,
    )?;
    let short_spike_hash_match = short.hashes.spike_counts
        == value_string(&short_reference, "/full_hashes/spike_counts_sha256")?;
    let short_state_hash_match =
        short.hashes.state == value_string(&short_reference, "/full_hashes/state_sha256")?;

    let gate = evaluate_gate(&cases, &protocol.seeds)?;
    let reference_software_gate = value_bool(&reference_summary, "/software_controls_pass")?;
    let reference_pathway_gate =
        value_bool(&reference_summary, "/pathway_response_reproduced_all_seeds")?;
    let acceptance_pass = (!protocol.acceptance.cuda_repeats_exact || all_repeats_exact)
        && (!protocol
            .acceptance
            .population_totals_match_reference_exactly
            || all_population_matches)
        && (!protocol
            .acceptance
            .input_relay_and_each_motor_pool_match_reference_exactly
            || all_groups_match)
        && (!protocol.chunk_invariance.require_exact_spike_counts || chunk_spikes_exact)
        && (!protocol.chunk_invariance.require_exact_final_f32_state || chunk_state_exact)
        && (!protocol.short_reference.require_exact_spike_count_hash || short_spike_hash_match)
        && gate.software_controls_pass == protocol.acceptance.software_controls_pass
        && gate.software_controls_pass == reference_software_gate
        && gate.pathway_response_reproduced_all_seeds
            == protocol
                .acceptance
                .expected_pathway_response_reproduced_all_seeds
        && gate.pathway_response_reproduced_all_seeds == reference_pathway_gate;

    let report = json!({
        "schema": "flybrain.cuda-full-cns-validation-report",
        "schema_version": 1,
        "protocol": {
            "path": options.protocol,
            "sha256": protocol_sha256,
        },
        "engine": "rust-cuda",
        "body_behavior": "not_simulated",
        "pack": {
            "path": protocol.pack_path,
            "materialization": pack.materialization(),
            "neuron_count": pack.neuron_count(),
            "edge_count": pack.edge_count(),
            "contact_sum": pack.manifest.contact_sum,
            "array_sha256": pack.manifest.array_sha256,
        },
        "neural_io": {
            "path": protocol.neural_io_path,
            "sha256": protocol.neural_io_sha256,
            "group_count": neural_io.groups.len(),
        },
        "pathway": {
            "path": protocol.pathway_path,
            "sha256": protocol.pathway_sha256,
            "name": pathway.name,
            "anatomical_edge_count": resolved.anatomical_edge_count,
        },
        "model_parameters": parameters,
        "cases": cases.into_iter().map(|case| case.report).collect::<Vec<_>>(),
        "determinism": {
            "repeat_runs": protocol.repeat_runs,
            "all_repeat_hashes_exact": all_repeats_exact,
            "chunk_primary": protocol.chunk_steps,
            "chunk_alternate": protocol.chunk_invariance.alternate_chunk_steps,
            "chunk_spike_counts_exact": chunk_spikes_exact,
            "chunk_final_f32_state_exact": chunk_state_exact,
        },
        "short_metal_f32_reference": {
            "path": protocol.short_reference.path,
            "sha256": protocol.short_reference.sha256,
            "steps": protocol.short_reference.steps,
            "spike_count_hash_match": short_spike_hash_match,
            "state_hash_match_diagnostic": short_state_hash_match,
            "cuda_hashes": hashes_json(&short.hashes),
            "reference_hashes": short_reference["full_hashes"].clone(),
            "final_f32_state_hash_required": !protocol.short_reference.final_f32_state_hash_is_diagnostic,
        },
        "reference_comparison": {
            "population_totals_all_match": all_population_matches,
            "input_relay_motor_groups_all_match": all_groups_match,
            "first_difference": first_reference_difference,
        },
        "experimental_gate": gate.to_json(),
        "reference_gate": {
            "summary_path": protocol.reference_summary_path,
            "summary_sha256": protocol.reference_summary_sha256,
            "software_controls_pass": reference_software_gate,
            "pathway_response_reproduced_all_seeds": reference_pathway_gate,
        },
        "acceptance_policy": protocol.acceptance,
        "acceptance_pass": acceptance_pass,
    });
    write_json_create_new(&options.output, &serde_json::to_string_pretty(&report)?)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "output": options.output,
            "acceptance_pass": acceptance_pass,
            "all_repeat_hashes_exact": all_repeats_exact,
            "population_totals_all_match": all_population_matches,
            "input_relay_motor_groups_all_match": all_groups_match,
            "short_spike_count_hash_match": short_spike_hash_match,
            "short_state_hash_match_diagnostic": short_state_hash_match,
            "software_controls_pass": gate.software_controls_pass,
            "pathway_response_reproduced_all_seeds": gate.pathway_response_reproduced_all_seeds,
        }))?
    );
    if !acceptance_pass {
        bail!("full-CNS CUDA acceptance failed; report saved")
    }
    Ok(())
}

fn validate_protocol(protocol: &Protocol) -> Result<()> {
    if protocol.schema != "flybrain.cuda-full-cns-validation"
        || protocol.schema_version != 1
        || protocol.steps == 0
        || protocol.short_reference.steps == 0
        || !protocol.rate_hz.is_finite()
        || protocol.rate_hz < 0.0
        || protocol.seeds.is_empty()
        || protocol.controls.is_empty()
        || protocol.repeat_runs < 2
        || protocol.chunk_steps == 0
        || protocol.chunk_invariance.alternate_chunk_steps == 0
        || protocol
            .acceptance
            .long_run_cpu_f64_per_neuron_equality_required
        || protocol.acceptance.body_behavior_evaluated
    {
        bail!("invalid full-CNS CUDA validation protocol");
    }
    let expected_case_count = protocol
        .seeds
        .len()
        .checked_mul(protocol.controls.len())
        .context("case count overflow")?;
    if protocol.reference_cases.len() != expected_case_count {
        bail!("reference case count does not match seeds × controls");
    }
    for seed in &protocol.seeds {
        for control in &protocol.controls {
            parse_control(control)?;
            if protocol
                .reference_cases
                .iter()
                .filter(|case| case.seed == *seed && case.control == *control)
                .count()
                != 1
            {
                bail!("each seed/control pair must have exactly one reference case");
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_case(
    pack: &ConnectomePack,
    neural_io: &NeuralIoResolution,
    pathway: &ResolvedPathway,
    parameters: ModelParameters,
    seed: u64,
    control: PathwayControl,
    steps: usize,
    rate_hz: f64,
    chunk_steps: usize,
) -> Result<ComputedCase> {
    let schedule = pathway.schedule(
        control,
        steps,
        rate_hz,
        seed,
        parameters,
        pack.neuron_count(),
    )?;
    let driven = if control == PathwayControl::RelayDriven {
        &pathway.relay_indices
    } else {
        &pathway.stimulus_indices
    };
    let construction_started = Instant::now();
    let mut engine = CudaEngine::new(
        pack,
        parameters,
        None,
        None,
        driven,
        pathway.silenced_sources(control),
    )?;
    let construction_seconds = construction_started.elapsed().as_secs_f64();
    let device_name = engine.device_name().to_owned();
    let allocated_bytes = engine.allocated_bytes();
    let run = engine.run_schedule(&schedule, chunk_steps)?;
    let duration_seconds = steps as f64 * parameters.dt_ms / 1000.0;
    let input = counts_for(&pathway.stimulus_indices, &run.spike_counts);
    let relay = counts_for(&pathway.relay_indices, &run.spike_counts);
    let readouts: BTreeMap<_, _> = pathway
        .readout_indices
        .iter()
        .map(|(name, indices)| (name.clone(), counts_for(indices, &run.spike_counts)))
        .collect();
    let population = counts_for_all(&run.spike_counts);
    let hashes = hashes(&run);
    let io_groups: BTreeMap<_, _> = neural_io
        .groups
        .iter()
        .map(|(name, group)| {
            (
                name.clone(),
                counts_json(
                    &counts_for(&group.engine_indices, &run.spike_counts),
                    duration_seconds,
                    false,
                ),
            )
        })
        .collect();
    let control_name = control_name(control).to_owned();
    let report = json!({
        "seed": seed,
        "control": control_name,
        "steps": steps,
        "duration_ms": duration_seconds * 1000.0,
        "rate_hz": rate_hz,
        "stimulus_event_count": schedule.counts().iter().filter(|&&count| count != 0).count(),
        "input": counts_json(&input, duration_seconds, true),
        "relay": counts_json(&relay, duration_seconds, true),
        "readouts": readouts.iter().map(|(name, counts)| {
            (name.clone(), counts_json(counts, duration_seconds, true))
        }).collect::<BTreeMap<_, _>>(),
        "population": counts_json(&population, duration_seconds, false),
        "neural_io_groups": io_groups,
        "hashes": hashes_json(&hashes),
        "timing": {
            "construction_seconds": construction_seconds,
            "run_seconds": run.elapsed.as_secs_f64(),
            "biological_seconds": duration_seconds,
            "simulation_realtime_factor": duration_seconds / run.elapsed.as_secs_f64(),
            "device_name": device_name,
            "allocated_bytes": allocated_bytes,
            "chunk_steps": chunk_steps,
        },
    });
    Ok(ComputedCase {
        seed,
        control: control_name,
        input,
        relay,
        readouts,
        population,
        hashes,
        report,
    })
}

struct Comparison {
    population_match: bool,
    groups_match: bool,
    spike_hash_match: bool,
    first_difference: Option<Value>,
    first_difference_for_report: Option<Value>,
}

fn compare_reference(case: &ComputedCase, reference: &Value) -> Result<Comparison> {
    if value_u64(reference, "/seed")? != case.seed
        || value_string(reference, "/control")? != case.control
    {
        bail!("reference seed/control does not match computed case");
    }
    let expected_population_total = value_u64(reference, "/population/total_spikes")?;
    let expected_population_active = value_u64(reference, "/population/active_neurons")? as usize;
    let population_match = case.population.total == expected_population_total
        && case.population.active == expected_population_active;
    let mut difference = if population_match {
        None
    } else {
        Some(json!({
            "seed": case.seed,
            "control": case.control,
            "field": "population",
            "actual_total": case.population.total,
            "expected_total": expected_population_total,
            "actual_active": case.population.active,
            "expected_active": expected_population_active,
        }))
    };

    let mut groups_match = compare_counts(
        &case.input,
        reference
            .pointer("/inputs/spike_counts")
            .context("reference inputs missing")?,
        "inputs",
        case,
        &mut difference,
    )?;
    groups_match &= compare_counts(
        &case.relay,
        reference
            .pointer("/relays/spike_counts")
            .context("reference relays missing")?,
        "relays",
        case,
        &mut difference,
    )?;
    for (name, counts) in &case.readouts {
        let pointer = format!("/readouts/{name}/spike_counts");
        groups_match &= compare_counts(
            counts,
            reference
                .pointer(&pointer)
                .with_context(|| format!("reference {name} missing"))?,
            name,
            case,
            &mut difference,
        )?;
    }
    let spike_hash_match =
        case.hashes.spike_counts == value_string(reference, "/full_hashes/spike_counts_sha256")?;
    Ok(Comparison {
        population_match,
        groups_match,
        spike_hash_match,
        first_difference: difference.clone(),
        first_difference_for_report: difference,
    })
}

fn compare_counts(
    actual: &Counts,
    expected: &Value,
    group: &str,
    case: &ComputedCase,
    difference: &mut Option<Value>,
) -> Result<bool> {
    let expected: Vec<u32> = serde_json::from_value(expected.clone())?;
    let matches = actual.per_neuron == expected;
    if !matches && difference.is_none() {
        let index = actual
            .per_neuron
            .iter()
            .zip(&expected)
            .position(|(actual, expected)| actual != expected)
            .unwrap_or(actual.per_neuron.len().min(expected.len()));
        *difference = Some(json!({
            "seed": case.seed,
            "control": case.control,
            "field": "group_spike_count",
            "group": group,
            "group_index": index,
            "actual": actual.per_neuron.get(index),
            "expected": expected.get(index),
        }));
    }
    Ok(matches)
}

struct Gate {
    rows: Vec<Value>,
    checks: Vec<Value>,
    software_controls_pass: bool,
    pathway_response_reproduced_all_seeds: bool,
}

impl Gate {
    fn to_json(&self) -> Value {
        json!({
            "rows": self.rows,
            "checks": self.checks,
            "software_controls_pass": self.software_controls_pass,
            "pathway_response_reproduced_all_seeds": self.pathway_response_reproduced_all_seeds,
            "interpretation": "Computational intervention check only; body behavior is not simulated.",
        })
    }
}

fn evaluate_gate(cases: &[ComputedCase], seeds: &[u64]) -> Result<Gate> {
    let mut rows = Vec::new();
    for case in cases {
        rows.push(json!({
            "seed": case.seed,
            "control": case.control,
            "input_spikes": case.input.total,
            "relay_spikes": case.relay.total,
            "motor_spikes": case.readouts.values().map(|counts| counts.total).sum::<u64>(),
            "population_spikes": case.population.total,
        }));
    }
    let mut checks = Vec::new();
    let mut software_controls_pass = true;
    let mut pathway_response_reproduced_all_seeds = true;
    for &seed in seeds {
        let intact = find_case(cases, seed, "intact")?;
        let no_input = find_case(cases, seed, "no-input")?;
        let input_disconnected = find_case(cases, seed, "input-disconnected")?;
        let relay_disconnected = find_case(cases, seed, "relay-disconnected")?;
        let intact_motor = intact
            .readouts
            .values()
            .map(|counts| counts.total)
            .sum::<u64>();
        let disconnected_motor = relay_disconnected
            .readouts
            .values()
            .map(|counts| counts.total)
            .sum::<u64>();
        let no_input_is_quiet = no_input.population.total == 0;
        let input_disconnect_preserves_stimulus = intact.input == input_disconnected.input;
        let input_disconnect_blocks_all_downstream_spikes = input_disconnected.relay.total == 0
            && input_disconnected
                .readouts
                .values()
                .all(|counts| counts.total == 0)
            && input_disconnected.population.total == input_disconnected.input.total;
        let intact_reaches_relay_and_motors = intact.relay.total > 0 && intact_motor > 0;
        let both_relays_active = intact.relay.per_neuron.iter().all(|&count| count > 0);
        let relay_disconnect_reduces_motor_spikes = disconnected_motor < intact_motor;
        software_controls_pass &= no_input_is_quiet
            && input_disconnect_preserves_stimulus
            && input_disconnect_blocks_all_downstream_spikes
            && intact_reaches_relay_and_motors
            && both_relays_active;
        pathway_response_reproduced_all_seeds &= relay_disconnect_reduces_motor_spikes;
        checks.push(json!({
            "seed": seed,
            "no_input_is_quiet": no_input_is_quiet,
            "input_disconnect_preserves_stimulus": input_disconnect_preserves_stimulus,
            "input_disconnect_blocks_all_downstream_spikes": input_disconnect_blocks_all_downstream_spikes,
            "intact_reaches_relay_and_motors": intact_reaches_relay_and_motors,
            "both_relays_active": both_relays_active,
            "all_six_motor_pools_active": intact.readouts.values().all(|counts| counts.total > 0),
            "all_twelve_motor_neurons_active": intact.readouts.values().all(|counts| counts.per_neuron.iter().all(|&count| count > 0)),
            "relay_disconnect_reduces_motor_spikes": relay_disconnect_reduces_motor_spikes,
        }));
    }
    Ok(Gate {
        rows,
        checks,
        software_controls_pass,
        pathway_response_reproduced_all_seeds,
    })
}

fn find_case<'a>(cases: &'a [ComputedCase], seed: u64, control: &str) -> Result<&'a ComputedCase> {
    cases
        .iter()
        .find(|case| case.seed == seed && case.control == control)
        .with_context(|| format!("missing computed case seed={seed} control={control}"))
}

fn counts_for(indices: &[u32], spike_counts: &[u32]) -> Counts {
    let per_neuron: Vec<_> = indices
        .iter()
        .map(|&index| spike_counts[index as usize])
        .collect();
    counts_for_all(&per_neuron)
}

fn counts_for_all(per_neuron: &[u32]) -> Counts {
    Counts {
        per_neuron: per_neuron.to_vec(),
        total: per_neuron.iter().map(|&count| u64::from(count)).sum(),
        active: per_neuron.iter().filter(|&&count| count != 0).count(),
    }
}

fn counts_json(counts: &Counts, duration_seconds: f64, include_per_neuron: bool) -> Value {
    let mut value = json!({
        "neuron_count": counts.per_neuron.len(),
        "total_spikes": counts.total,
        "active_neurons": counts.active,
        "mean_rate_hz": counts.total as f64 / duration_seconds / counts.per_neuron.len() as f64,
    });
    if include_per_neuron {
        value["spike_counts"] = json!(counts.per_neuron);
    }
    value
}

fn hashes(run: &CudaRun) -> Hashes {
    let spike_counts = sha256_u32(&run.spike_counts);
    let voltage = sha256_f32(&run.voltage_mv);
    let conductance = sha256_f32(&run.conductance_mv);
    let mut digest = Sha256::new();
    for value in &run.voltage_mv {
        digest.update(value.to_bits().to_le_bytes());
    }
    for value in &run.conductance_mv {
        digest.update(value.to_bits().to_le_bytes());
    }
    Hashes {
        spike_counts,
        voltage,
        conductance,
        state: format!("{:x}", digest.finalize()),
    }
}

fn hashes_json(hashes: &Hashes) -> Value {
    json!({
        "spike_counts_sha256": hashes.spike_counts,
        "voltage_final_mv_sha256": hashes.voltage,
        "conductance_final_mv_sha256": hashes.conductance,
        "state_sha256": hashes.state,
    })
}

fn sha256_u32(values: &[u32]) -> String {
    let mut digest = Sha256::new();
    for value in values {
        digest.update(value.to_le_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn sha256_f32(values: &[f32]) -> String {
    let mut digest = Sha256::new();
    for value in values {
        digest.update(value.to_bits().to_le_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn read_verified_json_bytes(path: &Path, expected_sha256: Option<&str>) -> Result<Vec<u8>> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    if let Some(expected) = expected_sha256 {
        let actual = sha256_bytes(&bytes);
        if actual != expected {
            bail!(
                "SHA256 mismatch for {}: expected {expected}, got {actual}",
                path.display()
            );
        }
    }
    serde_json::from_slice::<Value>(&bytes)
        .with_context(|| format!("validating JSON {}", path.display()))?;
    Ok(bytes)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn value_u64(value: &Value, pointer: &str) -> Result<u64> {
    value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .with_context(|| format!("reference field {pointer} is not a u64"))
}

fn value_string(value: &Value, pointer: &str) -> Result<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .with_context(|| format!("reference field {pointer} is not a string"))
}

fn value_bool(value: &Value, pointer: &str) -> Result<bool> {
    value
        .pointer(pointer)
        .and_then(Value::as_bool)
        .with_context(|| format!("reference field {pointer} is not a bool"))
}

fn parse_control(value: &str) -> Result<PathwayControl> {
    match value {
        "intact" => Ok(PathwayControl::Intact),
        "no-input" => Ok(PathwayControl::NoInput),
        "input-disconnected" => Ok(PathwayControl::InputDisconnected),
        "relay-disconnected" => Ok(PathwayControl::RelayDisconnected),
        "relay-driven" => Ok(PathwayControl::RelayDriven),
        _ => bail!("unsupported pathway control {value:?}"),
    }
}

fn control_name(control: PathwayControl) -> &'static str {
    match control {
        PathwayControl::Intact => "intact",
        PathwayControl::NoInput => "no-input",
        PathwayControl::InputDisconnected => "input-disconnected",
        PathwayControl::RelayDisconnected => "relay-disconnected",
        PathwayControl::RelayDriven => "relay-driven",
    }
}

fn write_json_create_new(path: &Path, serialized: &str) -> Result<()> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if !parent.is_dir() {
        bail!("output parent is not a directory: {}", parent.display());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("creating {}", path.display()))?;
    file.write_all(serialized.as_bytes())?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}
