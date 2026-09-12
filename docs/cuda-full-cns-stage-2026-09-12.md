# 完整 MaleCNS CUDA 阶段报告

日期：2026-09-12。环境：WSL2/Linux、NVIDIA GeForce RTX 5080、CUDA Toolkit
13.0。此阶段只验证独立 CUDA 神经后端，不接 `BrainBodyBridge`、MuJoCo、world 或行为层。
MaleCNS 数据、神经 I/O、sensory/motor mapping、CSR、模型参数和实验刺激均未修改。

## 固定协议与资源

验收协议在运行前写入
`experiments/male_cns_cuda_validation_v1.json`，SHA-256 为
`9a981b56f68da74b5a8e008540d51197da5a939113ad1477cf5eae9563e816e2`。
协议绑定了 neural-I/O、MeVP24–DNp10 pathway、Metal 参考 summary，以及 15 个参考
case 的路径和哈希。测试使用 3 个 seed、5 个控制条件、每个 case 2 次独立运行、
200 ms/2,000 tick、150 Hz 固定刺激；另测 256/37 tick 窗口划分和 20 ms 短程参考。

实际载入的 pack 为 `male-cns-v1.0-superclass-non-null-known-nt`：166,700 个神经元、
24,469,412 条有向边、120,260,398 个 contact。四个 CSR/ID 数组哈希由结果文件记录，
设备常驻分配为 154,818,076 bytes。

## 验收结果

完整验收通过：

- 同一 case 两次新建 CUDA 引擎的 spike count 与最终 `f32` 状态哈希逐位一致；
- 256 与 37 tick 分块的 spike count 与最终 `f32` 状态哈希逐位一致；
- 15 个 case 的全 CNS population total/active、输入、relay，以及六个 motor pool 的
  逐神经元 spike counts 全部与既有 Metal 参考完全一致；首次差异为 `null`；
- 作为额外诊断，15 个 case 的全 166,700 神经元 spike-count 哈希也全部一致；
- 20 ms CUDA 与 Metal 的 spike-count、voltage、conductance 和组合状态哈希全部一致；
- software controls 通过；已知 MeVP24–DNp10 pathway 生物学门控仍为失败，未被 CUDA
  移植错误地改成通过。relay disconnect 只在 seed 1 降低 motor spikes，在 seed 2/3
  不降低，和历史结果一致；
- 此阶段没有运行身体，因此不对飞行、舔糖、清洁或 MuJoCo 行为作任何结论。

结果文件为 `outputs/cuda/male_cns_validation_2026-09-12.json`，本次文件 SHA-256 为
`e0c6ea8d6454a0169b5c0d5caddb07e173bef90248ffed1865b878a8728d7234`。

## 初步性能

三个 intact case 各模拟 0.2 biological seconds，神经计算分别耗时 0.17535、0.16035、
0.15379 秒，平均 0.16316 秒，即约 `1.23×` 实时。首个 case 的引擎构建/CSR 上传为
0.19372 秒，后两个为约 0.017 秒；该一次性成本不计入上述神经运行速度。不同 control
的活动量不同，全部 case 的运行时间范围为 0.02960–0.18948 秒，不能用 no-input 的
低耗时代表正常闭环性能。

这是隔离神经核心的首轮 release 测量，不包含 bridge、原生 MuJoCo、显示和感官生成，
也尚未优化 active-source queue 或 CUDA Graph。它证明当前核心在正常 intact 刺激下已经
超过实时，但不能据此外推完整闭环速度；下一阶段必须分别测量神经、bridge、物理和显示。

## 复现

```bash
cargo build --release --features cuda --bin flybrain-cuda-validate
target/release/flybrain-cuda-validate \
  --protocol experiments/male_cns_cuda_validation_v1.json \
  --output outputs/cuda/male_cns_validation_2026-09-12.json
```

验证器采用 create-new 输出语义，防止无意覆盖已有证据；复跑时应指定新的输出路径。
下一阶段才开始把 `CudaEngine` 接入保持原语义的 `BrainBodyBridge` 和原生 MuJoCo。
