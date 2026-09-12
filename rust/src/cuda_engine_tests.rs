use super::{CudaEngine, CudaStep};
use crate::fixture::TickFixture;
use crate::pack::ConnectomePack;
use crate::parameters::ModelParameters;
use crate::stimulus::EventSchedule;

fn fixture_trace() -> (Vec<CudaStep>, String) {
    let fixture = TickFixture::load("fixtures/tiny-parity-v1.json").unwrap();
    let connectome = fixture.connectome().unwrap();
    let schedule = fixture.event_schedule().unwrap();
    let mut engine = CudaEngine::new(
        &connectome,
        fixture.parameters,
        Some(&fixture.initial_state.v_mv),
        Some(&fixture.initial_state.g_mv),
        &fixture.overrides.zero_refractory,
        &fixture.overrides.silenced_sources,
    )
    .unwrap();
    let device = engine.device_name().to_owned();
    (engine.run_recorded(&schedule).unwrap(), device)
}

#[test]
fn cuda_matches_brian_fixture_spikes_and_short_f32_tolerance() {
    let fixture = TickFixture::load("fixtures/tiny-parity-v1.json").unwrap();
    let (trace, device) = fixture_trace();
    let (repeat, repeat_device) = fixture_trace();
    assert_eq!(device, repeat_device);
    assert_eq!(trace, repeat, "CUDA trace changed between identical runs");

    let mut actual_spikes = Vec::new();
    let mut first_numeric_difference = None;
    let mut max_voltage_difference = 0.0_f64;
    let mut max_conductance_difference = 0.0_f64;
    for (tick, state) in trace.iter().enumerate() {
        for (neuron, &fired) in state.spikes.iter().enumerate() {
            if fired != 0 {
                actual_spikes.push((tick, neuron));
            }
            for (field, actual, expected) in [
                (
                    "voltage",
                    f64::from(state.voltage_mv[neuron]),
                    fixture.expected.v_end_mv[tick][neuron],
                ),
                (
                    "conductance",
                    f64::from(state.conductance_mv[neuron]),
                    fixture.expected.g_end_mv[tick][neuron],
                ),
            ] {
                let difference = (actual - expected).abs();
                if difference != 0.0 && first_numeric_difference.is_none() {
                    first_numeric_difference = Some((tick, neuron, field, actual, expected));
                }
                if field == "voltage" {
                    max_voltage_difference = max_voltage_difference.max(difference);
                } else {
                    max_conductance_difference = max_conductance_difference.max(difference);
                }
                assert!(
                    difference <= 1e-5,
                    "first over-tolerance state at tick {tick}, neuron {neuron}, {field}: CUDA={actual}, reference={expected}, abs={difference}"
                );
            }
        }
    }
    let expected_spikes: Vec<_> = fixture
        .expected
        .spike_events
        .iter()
        .map(|event| (event.tick, event.neuron))
        .collect();
    assert_eq!(actual_spikes, expected_spikes, "spike event divergence");
    println!(
        "device={device}; first_numeric_difference={first_numeric_difference:?}; max_voltage_abs={max_voltage_difference:e}; max_conductance_abs={max_conductance_difference:e}; spike_divergence=none"
    );
}

#[test]
fn cuda_chunk_boundaries_do_not_change_final_state() {
    let fixture = TickFixture::load("fixtures/tiny-parity-v1.json").unwrap();
    let connectome = fixture.connectome().unwrap();
    let schedule = fixture.event_schedule().unwrap();
    let create = || {
        CudaEngine::new(
            &connectome,
            fixture.parameters,
            Some(&fixture.initial_state.v_mv),
            Some(&fixture.initial_state.g_mv),
            &fixture.overrides.zero_refractory,
            &fixture.overrides.silenced_sources,
        )
        .unwrap()
    };
    let mut single_chunk = create();
    let mut uneven_chunks = create();
    let first = single_chunk
        .run_schedule(&schedule, schedule.steps())
        .unwrap();
    let second = uneven_chunks.run_schedule(&schedule, 4).unwrap();
    assert_eq!(first.spike_counts, second.spike_counts);
    assert_eq!(first.voltage_mv, second.voltage_mv);
    assert_eq!(first.conductance_mv, second.conductance_mv);
}

#[test]
fn cuda_preserves_zero_delay_signed_delay_silencing_and_refractory_order() {
    for (delay_ms, expected_tick) in [(0.0, 0_usize), (0.2, 2)] {
        for signed_count in [4_i16, -4_i16] {
            let connectome =
                ConnectomePack::from_arrays([10, 20], [0, 1, 1], [1], [signed_count]).unwrap();
            let parameters = ModelParameters {
                delay_ms,
                ..ModelParameters::default()
            };
            let schedule = EventSchedule::empty(4, 2);
            let mut engine = CudaEngine::new(
                &connectome,
                parameters,
                Some(&[-44.0, -52.0]),
                None,
                &[],
                &[],
            )
            .unwrap();
            let trace = engine.run_recorded(&schedule).unwrap();
            for (tick, state) in trace.iter().enumerate() {
                let expected = if tick == expected_tick {
                    f32::from(signed_count) * 0.275
                } else if tick < expected_tick {
                    0.0
                } else {
                    state.conductance_mv[1]
                };
                if tick <= expected_tick {
                    assert_eq!(state.conductance_mv[1], expected);
                }
            }
        }
    }

    let connectome = ConnectomePack::from_arrays([10, 20], [0, 1, 1], [1], [4]).unwrap();
    let schedule = EventSchedule::empty(4, 2);
    let mut silenced = CudaEngine::new(
        &connectome,
        ModelParameters {
            delay_ms: 0.0,
            ..ModelParameters::default()
        },
        Some(&[-44.0, -52.0]),
        None,
        &[],
        &[0],
    )
    .unwrap();
    let trace = silenced.run_recorded(&schedule).unwrap();
    assert!(trace.iter().all(|state| state.conductance_mv[1] == 0.0));

    let isolated = ConnectomePack::from_arrays([10], [0, 0], [], []).unwrap();
    let parameters = ModelParameters {
        refractory_ms: 0.3,
        ..ModelParameters::default()
    };
    let schedule = EventSchedule::new(vec![0], vec![0, 1, 0, 0], 4, 1).unwrap();
    let mut refractory =
        CudaEngine::new(&isolated, parameters, Some(&[-44.0]), None, &[], &[]).unwrap();
    let trace = refractory.run_recorded(&schedule).unwrap();
    assert_eq!(
        trace
            .iter()
            .map(|state| state.spikes[0])
            .collect::<Vec<_>>(),
        [1, 0, 0, 1]
    );
}

#[test]
fn cuda_sparse_dense_and_split_windows_are_exactly_equal() {
    let connectome = ConnectomePack::from_arrays([10_u64, 20], [0_u32, 0, 0], [], []).unwrap();
    let parameters = ModelParameters::default();
    let dense =
        EventSchedule::new(vec![0, 1], vec![1, 0, 0, 0, 0, 2, 0, 0, 1, 0, 0, 0], 6, 2).unwrap();
    let mut dense_engine =
        CudaEngine::new(&connectome, parameters, None, None, &[0, 1], &[]).unwrap();
    let dense_window = dense_engine.run_window(&dense, &[0, 1]).unwrap();
    let dense_state = dense_engine
        .run_schedule(&EventSchedule::empty(0, 2), 1)
        .unwrap();

    let mut sparse_engine =
        CudaEngine::new(&connectome, parameters, None, None, &[0, 1], &[]).unwrap();
    let sparse_window = sparse_engine
        .run_window_sparse(6, &[0, 1, 1, 2, 2, 3, 3], &[0, 1, 0], &[1, 2, 1], &[0, 1])
        .unwrap();
    let sparse_state = sparse_engine
        .run_schedule(&EventSchedule::empty(0, 2), 1)
        .unwrap();

    let mut split_engine =
        CudaEngine::new(&connectome, parameters, None, None, &[0, 1], &[]).unwrap();
    let first = split_engine
        .run_window_sparse(3, &[0, 1, 1, 2], &[0, 1], &[1, 2], &[0, 1])
        .unwrap();
    let second = split_engine
        .run_window_sparse(3, &[0, 0, 1, 1], &[0], &[1], &[0, 1])
        .unwrap();
    let split_state = split_engine
        .run_schedule(&EventSchedule::empty(0, 2), 1)
        .unwrap();

    assert_eq!(
        dense_window.spike_count_deltas,
        sparse_window.spike_count_deltas
    );
    assert_eq!(
        dense_window.spike_count_deltas,
        first
            .spike_count_deltas
            .iter()
            .zip(second.spike_count_deltas)
            .map(|(first, second)| first + second)
            .collect::<Vec<_>>()
    );
    assert_eq!(dense_state.spike_counts, sparse_state.spike_counts);
    assert_eq!(dense_state.spike_counts, split_state.spike_counts);
    assert_eq!(dense_state.voltage_mv, sparse_state.voltage_mv);
    assert_eq!(dense_state.voltage_mv, split_state.voltage_mv);
    assert_eq!(dense_state.conductance_mv, sparse_state.conductance_mv);
    assert_eq!(dense_state.conductance_mv, split_state.conductance_mv);
    assert!(
        sparse_engine
            .run_window_sparse(1, &[0, 2], &[0, 0], &[1, 1], &[])
            .is_err()
    );
}
