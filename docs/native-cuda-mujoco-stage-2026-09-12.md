# Linux CUDA + 原生 MuJoCo 接入报告

日期：2026-09-12。环境：WSL2、RTX 5080、CUDA Toolkit 13.0、MuJoCo 3.9.0、
Rust 1.96.0。本阶段将已经验收的 `CudaEngine` 接入原有 `BrainBodyBridge` 和
`SimulationStepper`，并在 Linux 启用现有原生 MuJoCo/world/viewer 模块。没有使用
浏览器或 WASM，也没有修改 MaleCNS 数据、neural-I/O、sensory/motor mapping、世界、
行为逻辑、模型参数、步态或翼运动。

## 实现边界

- Linux + `cuda` feature 下，`BrainBodyBridge` 的 `NeuralEngine` 指向 `CudaEngine`；
  macOS Metal 和 Emscripten WebGPU 选择保持不变。
- CUDA 遥测错误通过现有 `anyhow::Result` 返回，不静默吞掉设备读回失败。
- Linux 构建使用 vendored `mujoco-rs` 和专用 `flybrain` Conda 环境中的 MuJoCo 3.9.0；
  `tools/setup_mujoco_runtime.py` 创建项目内动态库符号链接，不复制或修改 Conda 库。
- `flybrain-world` 的 headless commands、offscreen renderer 和 WSLg viewer 均可在
  Linux/CUDA 下构建。GLFW 使用 Conda 包的 X11 动态库。

## 实测结果

原生世界资产检查返回 MuJoCo timestep `0.0001 s`、133 qpos、132 dofs、71 bodies、
127 joints、56 actuators 和 6 sensors。100-step neutral trace 完成并生成有限的 qpos/qvel。

项目既有的 2 秒 `tools/verify_cns_world.py` 成对门控通过，验收脚本未修改：

- intact：2,071,162 population spikes、8,345 motor-pool spikes，飞行 1.822 s，
  前向飞行积分 29.142 mm，最大边界越界为 0；
- motor-output-disconnected：2,072,049 population spikes、8,612 motor-pool spikes
  仍存在，但 motor commands 为零且全程保持地面；
- 两个条件的初始状态哈希一致，末位置相差 15.475 mm；所有单项检查和 paired lesion
  comparison 均通过。

额外的 2 秒 no-sensory-input run 得到 0 population spikes、0 motor-pool spikes、0 秒
飞行，符合无隐藏活动调度的预期。匹配 intact 初态的 olfactory-evoked-input disconnect
仍保留 2,072,123 population spikes 和 8,631 motor-pool spikes，但关闭 ORN evoked input
后产生不同闭环轨迹；这只是接口断连证据，不替代后续 20 秒取食/嗅觉门控。

WSLg viewer 以 640×480 实际打开，识别 166,700 个 MaleCNS 神经元、2,459/2,459
present neural-I/O selections 和 RTX 5080，并正常自动结束短跑。

证据文件：

- `outputs/cuda/cns-native-world-verification-2026-09-12.json`，SHA-256
  `eb53d5548edf573730d45a562e7630b22f89cccadae3ca8af6c495359c57a4ec`；
- `outputs/cuda/cns-native-no-sensory-2026-09-12.json`，SHA-256
  `a6d6054fc44eceddf3c9376d1d31688c97084511c3e73d6b7f9bbbc563ca2afc`；
- `outputs/cuda/cns-native-no-olfactory-evoked-settle05-2026-09-12.json`，SHA-256
  `1bfe2ee6ba969a6317aff1baa4708b8cea461588677874952ba5c895af9d3454`。

## 当前性能结论

0.2 biological seconds 的 intact `cns-check` 耗时 0.7375 s，约 `0.271×` 实时。
这比隔离 CUDA 核心的约 `1.23×` 慢，当前不能称完整闭环已达到实时。代码检查确认，
兼容版 CUDA `run_window_sparse()` 每个 2 ms 窗口会在运行前后读取完整 166,700-neuron
state 来计算 probe deltas，开启 population/brain telemetry 时还会增加完整读回。
因此当前结果主要验证功能接线，而不是最终 CUDA 性能。

下一小阶段应在不改变神经执行顺序的前提下新增 device-side probe delta 和 population
reduction，只读回 bridge 实际需要的少量值；然后重新跑 CUDA parity、上述成对门控并
分别测量 neural kernel/transfer、bridge、MuJoCo 和 viewer。20 秒取食/嗅觉实验在该
优化和回归通过后再运行。

## 复现命令

```bash
conda run -n flybrain python tools/setup_mujoco_runtime.py
cargo build --release --features cuda --bin flybrain-world
target/release/flybrain-world inspect --assets assets/neuromechfly
target/release/flybrain-world verify --assets assets/neuromechfly --steps 100
conda run -n flybrain python tools/verify_cns_world.py \
  --rust-bin target/release/flybrain-world \
  --cns-pack outputs/packs/male_cns_v1 \
  --duration-seconds 2 --control-hz 500 --start-food-distance 40 \
  --output outputs/cuda/cns-native-world-verification-2026-09-12.json
```
