# CUDA neural core stage report

Date: 2026-09-12. Target: WSL2/Linux, NVIDIA GeForce RTX 5080, driver 616.92,
CUDA Toolkit 13.0 (`nvcc 13.0.88`). This stage implements and validates only the
standalone neural core. It does not connect CUDA to `BrainBodyBridge`, MuJoCo,
the world, rendering, or the complete MaleCNS pack.

## Implemented scope

- `CudaEngine` is a Rust API behind the opt-in `cuda` Cargo feature on Linux.
- CUDA owns persistent CSR, `f32` state, refractory state, spike counters,
  signed `i32` arrivals, silencing flags, and the delay ring in device memory.
- CUDA kernels preserve the existing strict threshold, refractory decrement,
  decay, propagation, external-input, arrival application, reset, and ring-store order.
- Signed edge counts remain `i16`; deterministic atomic accumulation remains `i32`;
  model parameters are converted to the same `f32` values used by Metal.
- Dense schedules, sparse windows, probe deltas, recorded state, chunked execution,
  device identity, allocation accounting, and aggregate telemetry APIs are present.
- The first implementation finalizes each tick explicitly instead of using Metal's
  cross-tick `reset_decay_threshold` fusion. This preserves state order but is not a
  performance claim; active-source queues, CUDA Graphs, and full-CNS optimization are deferred.

The Rust build script invokes the system `nvcc` only when `--features cuda` is
enabled. The default target is `sm_120`, overrideable with `FLYBRAIN_CUDA_ARCH`.
`cuobjdump` confirmed that the produced archive contains
`libflybrain_cuda.1.sm_120.cubin`. No PyTorch, cuDNN, NVRTC, or new Rust dependency
was added.

## Short-run results

Command:

```bash
cargo test --lib --features cuda cuda_engine::tests -- --nocapture
```

All four CUDA tests passed on the RTX 5080, both serially and with the Rust test
harness's default parallel execution:

1. repeated fresh CUDA runs produced exactly equal `f32` traces and spike events;
2. one 15-tick chunk and uneven four-tick chunks produced exactly equal final
   counts, voltage, and conductance;
3. dense, sparse, and split sparse windows produced exactly equal final state and
   probe deltas, including continued delay-ring state across calls;
4. zero delay, two-tick delay, excitatory/inhibitory signed counts, silenced sources,
   and refractory/external-input ordering passed targeted fixtures.

Against the Brian2-validated float64 tiny fixture, the first nonzero numerical
difference occurs at tick 2, neuron 1, conductance: CUDA `13.75 mV` versus reference
`13.750000000000002 mV`. This is the expected representation difference after an
integer arrival is multiplied by the `f32` synaptic weight. Maximum differences
over the 15-tick trace were `3.907754056342583e-6 mV` for voltage and
`9.748347178373251e-7 mV` for conductance, both below the predetermined `1e-5 mV`
short-fixture tolerance. There was no spike divergence in the tested trace.

`cargo clippy --lib --features cuda -- -D warnings`, the CUDA-feature library build,
and the non-CUDA library build passed. A broad Linux library run passed 173 of 174
tests; the only failure was the existing `pinned_v783_artifact_resolves_against_local_pack`
test because `outputs/packs/flywire_v783` is not installed. The restored MaleCNS
pack tests passed. This missing unrelated v783 asset was not fabricated or replaced.

CUDA Compute Sanitizer could not run on this WSL/WDDM configuration because its
debugger interface reported the device unsupported. A host-side `cuda-gdb` trace
was used to locate an initial sparse-offset pointer bug; offsets are now kept on
the host and passed as scalar kernel arguments, after which serial and parallel
tests pass. Device sanitizer coverage therefore remains an explicit validation gap.

## Follow-up status

The unchanged full MaleCNS v1.0 pack, predeclared fixed replay, population responses,
determinism, memory and initial throughput were subsequently validated; see the
[full-CNS CUDA report](cuda-full-cns-stage-2026-09-12.md). Connecting `CudaEngine`
to `BrainBodyBridge` and native MuJoCo remains the next stage. Native MuJoCo is
mandatory for the WSL/Linux closed loop and remains outside this isolated core stage.
