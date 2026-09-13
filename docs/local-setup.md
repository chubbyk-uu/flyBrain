# Local development environment

Use a dedicated Conda environment named `flybrain`. Do not install or upgrade
dependencies in shared environments used by other projects.

From the repository root, on Linux/WSL2:

Use the existing shell `HTTP_PROXY`/`HTTPS_PROXY` settings for downloads, respecting
`NO_PROXY`. Do not commit proxy addresses or credentials, and do not change global
Conda proxy settings for this project.

```bash
conda create -n flybrain python=3.12 pip -y
conda activate flybrain
python -m pip install -e '.[reference,dev]' \
  'numpy==2.4.6' 'pyarrow==25.0.1' 'brian2==2.10.1' 'cython==3.1.3' \
  'pytest==9.1.0' 'ruff==0.16.0' 'mujoco==3.9.0' 'glfw==2.10.0'
python -m pip check
```

If the environment already exists, start with `conda activate flybrain`.
These explicit versions define the initial Linux environment; they are not a
complete transitive lock or an exact reproduction of the upstream `uv.lock`.
In particular, NumPy 2.4.6 is the version used for the initial local verification.
The Apple/MLX extra is not installed on Linux. PyTorch and a new FlyGym install
are not required for the existing exported body assets or the Rust CUDA backend.

Python supports data preparation and independent verification. The standalone
CUDA neural core and native embodied runtime now build on Linux. Prepare project-local
MuJoCo/GLFW links from the active dedicated environment before the first native build:

```bash
conda activate flybrain
python tools/setup_mujoco_runtime.py
cargo build --release --features cuda --bin flybrain-world
```

The setup script links MuJoCo 3.9.0 and the packaged X11 GLFW into
`work/mujoco/lib`; it does not copy or modify the Conda libraries. Cargo embeds a
project-relative runtime search path for binaries under `target/debug` and
`target/release`.
Keep the system CUDA Toolkit separate from Conda. WSL uses the Windows GPU driver;
do not install a Linux NVIDIA display driver inside WSL.

The standalone CUDA neural core uses the system `nvcc`, defaults to the RTX 5080
`sm_120` target, and does not require PyTorch or cuDNN:

```bash
cargo build --lib --features cuda
cargo test --lib --features cuda cuda_engine::tests -- --nocapture
```

The native sparse closed-loop path uses the stable direct CUDA submission mode by
default. The measured CUDA Graph experiment is opt-in and keeps direct mode as a
fallback:

```bash
FLYBRAIN_CUDA_EXECUTION=graph target/release/flybrain-world cns-check \
  --duration-seconds 1 --control-hz 500 --settle-seconds 0 \
  --output outputs/cuda/cns-graph-check.json
```

The neural and MuJoCo clocks are independently scheduled. Native `view` and `cns-check`
default to 0.1 ms neural and 0.2 ms MuJoCo ticks. Pass `--physics-dt-ms 0.1` for the
original reference physics timebase; reports record both tick counts under `timebase`.

Accepted values are `direct` and `graph`. Graph currently improves only the neural
portion slightly and is not the default; see the
[experiment report](cuda-graph-experiment-2026-09-12.md).

For a short native WSLg viewer run:

```bash
target/release/flybrain-world view --max-seconds 1
```

For the preferred Windows browser display, keep the native CUDA/MuJoCo simulation in WSL and start
it together with the static server:

```bash
tools/run_native_viewer.sh
```

Open `http://localhost:8080/native-view.html` in Windows Chrome/Edge. `web-view` defaults to native
MuJoCo 0.2 ms and a 30 Hz pose/snapshot stream on `ws://127.0.0.1:8765`; MaleCNS stays at 0.1 ms.
The first stage keeps native retina sensory capture and does not use the browser image for brain input.
The default `indoor-v2` scene now has a shallow sugar-liquid patch and a flat flower
on a low coffee table, not the earlier potted plant. The viewer also displays actual
aggregate neural telemetry. Manual reset changes the behavior seed and pauses;
the first ready viewer automatically resets once per backend process and continues.
Use `http://localhost:8080/native-gallery.html` to collect non-empty fixed room views and CNS-observed
behavior states. The normal viewer remains `native-view.html`; the gallery never sends commands.

Set `CUDA_HOME` only if the toolkit is not available at `/usr/local/cuda`. Set
`FLYBRAIN_CUDA_ARCH` to an explicit `nvcc -arch` value when building for a GPU
other than the current RTX 5080 development target.

Focused environment verification:

```bash
python -m pytest -q tests/test_male_cns.py tests/test_reference.py \
  tests/test_brian_parity.py tests/test_verify_cns_world.py
```

These tests do not establish full-pack neural execution or native GPU parity.
Some other tests require data omitted from Git or an accessible Metal device.

Both native and browser full-CNS modes require the local pack, which is not in Git.
Restore the published arrays verbatim and validate against the pinned I/O hashes:

```bash
python tools/restore_male_cns_pack.py
```

The restoration script refuses conflicting local content; do not regenerate I/O to
bypass a mismatch. For the existing browser bundle, after restoring the pack, run:

```bash
npm --prefix web start
```

Open `http://localhost:8080` in a Windows browser with WebGPU enabled and choose
`Start full CNS`. The browser path does not require Conda or CUDA. See
[browser runtime](browser.md) for the separate Emscripten source-build procedure.
