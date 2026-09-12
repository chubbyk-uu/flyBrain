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

Accepted values are `direct` and `graph`. Graph currently improves only the neural
portion slightly and is not the default; see the
[experiment report](cuda-graph-experiment-2026-09-12.md).

For a short native WSLg viewer run:

```bash
target/release/flybrain-world view --max-seconds 1
```

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

For the existing browser bundle, restore and verify the MaleCNS pack as described
in the [project specification](project-spec.md), then run:

```bash
npm --prefix web start
```

Open `http://localhost:8080` in a Windows browser with WebGPU enabled and choose
`Start full CNS`. The browser path does not require Conda or CUDA. See
[browser runtime](browser.md) for the separate Emscripten source-build procedure.
