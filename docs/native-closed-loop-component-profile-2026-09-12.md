# 原生闭环组件分项报告

日期：2026-09-12。环境：WSL2/Linux、NVIDIA GeForce RTX 5080、默认
`chunked-256` CUDA propagation、原生 MuJoCo 3.9。本阶段只增加墙钟观测字段，没有
修改 MaleCNS、神经更新、sensory/motor mapping、行为逻辑、模型参数、物理步长或世界。

## 计时边界

`SimulationStepper::step_window_steps` 现在记录每个控制窗口的总墙钟时间，以及其中的：

- `brain_wall_seconds`：sensory encoding、CUDA engine 和既有 brain decode/telemetry；
- `physics_wall_seconds`：每个 0.1 ms 物理步的 flight command、稳定器、翼控制、原生
  `mj_step` 和 flight telemetry；
- `non_brain_non_physics_seconds`：窗口总时间减去上述两项，包含其余感官采样、行为仲裁、
  导航和控制准备。

这些字段只在步骤完成后读取 `Instant`，不参与任何状态更新。

## 1 秒完整闭环结果

固定 1 biological second、500 Hz、无 settle，连续三次输出完全一致：population
`1,022,346`、motor `4,059`，最终位姿逐项相同。三次内部端到端为
`1.3084/1.2863/1.2397 s`，中位数 `1.2863 s`，约 `0.777×` 实时。

中位数分项：

- window wall：`1.2767 s`；
- physics/flight loop：`0.7543 s`，约占 window wall `59.1%`；
- brain wall：`0.5134 s`，约占 `40.2%`；
- 其中 CUDA engine：`0.4256 s`，sensory encoding：`0.0673 s`；
- 其余非 brain、非 physics：`0.0090 s`，约占 `0.7%`。

因此 `BrainBodyBridge` 和行为仲裁已经不是值得单独优化的主要部分。当前闭环是
physics/flight loop 与 CUDA brain 串行相加；保持 0.1 ms MuJoCo 步长和零额外感官延迟时，
不能靠调小 Rust 控制代码获得实时。

独立对照中，10,000 个 neutral MuJoCo step 的完整进程墙钟约 `1.21 s`，带稳定器和
飞行命令的 `flight-check` 完整进程墙钟约 `0.96 s`。它们的运动状态和计时边界不同，
只用于确认物理侧量级，不作为与闭环的严格 A/B。

## 回归

- 27 项 `world_sim` 测试全部通过；
- 2 秒默认 CUDA + 原生 MuJoCo 成对 world gate 通过，intact/disconnected 的 population、
  motor、飞行和位姿指标与计时前一致；报告为
  `outputs/cuda/cns-native-component-profile-world-verification-2026-09-12.json`，
  SHA-256：`26ffbd1d4a285032fc98506e03400f64aad38de9bcf00d9ff24ef7bc05c70adf`；
- `cargo check`、严格 Clippy 和 `git diff --check` 通过。

## 下一阶段边界

下一阶段针对每个 2 ms brain window 约 40–60 次 CUDA kernel launch 建立可回退的
CUDA Graph 实验路径，目标是把 brain wall 从约 `0.51 s` 压低到 `0.25 s` 左右，使
physics/flight loop 与 brain 串行总和接近实时。Graph 必须保持 20 个 neural tick、
delay-ring slot、external event 和 probe 的原顺序；先通过 tiny/parity，再进行完整 CNS
和 world gate。若实际收益不足，则不修改 MuJoCo timestep 来伪造性能提升，而是单独研究
原生物理模型的安全优化。
