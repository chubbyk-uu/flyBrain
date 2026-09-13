# FlyBrain — native CUDA cyber fly

This fork of [mehrantsi/flyBrain](https://github.com/mehrantsi/flyBrain) runs the
MaleCNS v1.0 brain + VNC model in a small indoor physical world. The preferred
runtime is Rust + CUDA + native MuJoCo on Linux/WSL2, with an independent Three.js
viewer in Windows Chrome/Edge. Metal and WebGPU/WASM remain reference paths.
The upstream online demo is not this fork's indoor scene.

## Run

Follow [local setup](docs/local-setup.md) for the dedicated Conda environment,
system CUDA Toolkit, MuJoCo libraries and pack restoration. PyTorch is not required.
From the repository root after setup:

```bash
conda activate flybrain
python tools/setup_mujoco_runtime.py
cargo build --release --locked --features cuda --bin flybrain-world
tools/run_native_viewer.sh --scene indoor-v2 --publish-hz 30 --speed 1
```

Open `http://localhost:8080/native-view.html?fps=60` in Windows Chrome/Edge.
The backend begins running immediately; the first ready viewer resets it once.
Refreshes and reconnects do not reset it again. Manual reset chooses a new behavior
seed and pauses; press resume to run. Use a fixed `--behavior-seed` for experiments.
Do not start a second backend on the same port. `--scene legacy` selects the old world.

Native simulation publishes poses/snapshots at 30 Hz. Browser interpolation and
rendering are independent, capped at 60 FPS by default (`?fps=90` is optional).
MuJoCo physics stays native at **0.2 ms**, neural ticks at **0.1 ms**, with a 500 Hz
brain/body exchange. Native binocular capture remains authoritative; browser
rendered images do not feed the neural network.

## Current behavior and limits

- A fresh room contains a low, long coffee table, a shallow irregular sugar-liquid
  patch and a flat flower on the tabletop; no pot, stem, leaves or floor mat.
- The hybrid controller supports walking, flight, landing, odor-guided feeding,
  hunger, flight fatigue, exploration and combined foreleg rubbing/head cleaning.
- The viewer shows needs, paired native eye previews and actual aggregate neural
  spike-rate telemetry. The graph is neither thought decoding nor literal EEG.
- The pack contains **166,700 neurons and 24,469,412 signed directed edges**.
  These model counts do not establish a complete biological brain copy.
- Sensory encoders, motor decoders, stabilization, needs and action coordination
  are engineering interfaces. Neural activity has tested causal roles; not every
  decision or joint trajectory emerges directly from the connectome.
- Vision supplies limited proxies, not recognition of sugar or flowers.
  Autonomous self-righting is deferred. Historical five-seed long-run acceptance
  remains incomplete; successful demos do not supersede failed gates.

See the [current specification](docs/project-spec.md) and [documentation index](docs/README.md).
Display FPS and simulation realtime factor are separate: short Windows runs reached
about 60 FPS, but throughput varies with workload/configuration. No universal realtime
guarantee is implied.

## Verification

```bash
npm --prefix web test
cargo test --release --locked --features cuda --lib
python -m pytest -q tests/test_male_cns.py tests/test_reference.py \
  tests/test_brian_parity.py tests/test_verify_cns_world.py
```

GPU tests require the local GPU/runtime and relevant data. Historical macOS/WebGPU
measurements are not current Linux results. CUDA acceptance requires self-determinism,
short fixture/spike parity and full-CNS population/causal gates, not long-horizon
per-neuron equality with CPU float64.

## Repository map

| Path | Purpose |
| --- | --- |
| `rust/src/`, `rust/cuda/`, `rust/shaders/` | Runtime and CUDA/Metal kernels |
| `web/` | Browser viewer, original WebGPU app and offline film studio |
| `assets/` | Versioned body, room and sensory assets; preserve manifests/licenses |
| `src/flybrain/`, `tests/`, `fixtures/` | Data preparation and independent references |
| `tools/`, `examples/` | Setup, diagnostics, acceptance and video production |
| `docs/` | Current guides and dated experiment evidence |
| `outputs/` | Local packs, recordings and generated artifacts; tracked evidence retained |

The final narrated 72-second portrait/landscape videos are described in
[video production](docs/video-production.md). Generated media are local artifacts,
not bundled in Git. See [artifact retention](docs/artifact-retention.md) before cleanup.

## Scientific sources and licenses

See [references](REFERENCES.md), [third-party notices](THIRD_PARTY_NOTICES.md) and
[license texts](licenses/README.md). Original code is MIT; datasets/assets have
separate licenses, including MaleCNS CC BY 4.0 and FlyWire v783 CC BY-NC 4.0.
The graph does not reconstruct donor memories, identity or complete physiology.
Legacy benchmark detail remains in Git history and the linked scientific/runtime
guides rather than being repeated here.
