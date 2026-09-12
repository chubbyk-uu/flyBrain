# 原生 CUDA 长程行为门控报告

日期：2026-09-12。环境：WSL2/Linux、NVIDIA GeForce RTX 5080、默认 direct CUDA、
`chunked-256` propagation、原生 MuJoCo 3.9。本阶段没有修改 MaleCNS 数据、
sensory/motor mapping、行为逻辑、模型参数、物理步长或世界。

## 协议

复用项目既有 `tools/verify_cns_odor_guidance.py` 预注册门控：

- intact：20 biological seconds；
- olfactory-evoked input disconnected：12 seconds，保留基线 ORN、其他感官和 motor；
- motor output disconnected：12 seconds，保留完整 CNS 运行和 spike 观测；
- 三组使用同一最终二进制、资产、初始位姿和参数，`control_hz=500`、默认 0.5 秒 settle、
  food distance 40 mm。

该验证器要求报告结构和时间线完整、初态一致、完整 CNS 持续活动、intact 嗅觉引导和
近食物状态出现、至少 1 秒连续且身体直立/有支撑/真实 taste/MN9/伸吻同时成立的 FEED，
以及餐后由神经输出驱动的离开。断连对照必须覆盖 intact 首次进食后的至少 1 秒。

## 结果

21 项预注册检查全部通过。

| 条件 | population spikes | motor spikes | 取食 | 飞行/结果 |
|---|---:|---:|---|---|
| intact 20 s | `21,543,617` | `80,716` | 248 个有效样本；连续 `2.964 s`，`6.602–9.566 s` | `11.018 s` 餐后离开；飞行 `14.190 s` |
| olfactory evoked disconnected 12 s | `12,580,771` | `51,342` | 无 | guidance/close 样本均为 0；其他 CNS 与 motor 活动保留 |
| motor output disconnected 12 s | `12,571,427` | `51,672` | 无，伸吻始终为 0 | 飞行始终为 0；CNS spikes 保留 |

三组初态 SHA-256 均为
`31504d7619258100bfa3ff76610e0940fcf2aabcc0ff68b10f4079750035dc23`。
这验证的是既有工程化 odor-guidance、身体控制和 MaleCNS I/O 接口之间的因果集成，
不是已经恢复真实果蝇的中央食物搜索策略。

验证汇总：
`outputs/cuda/cns-native-long-odor-guidance-verification-2026-09-12.json`，SHA-256：
`b6b0232e8e2168f477b025c6ec23546748de781e06865089e03485b01431c3e2`。

## 20 秒重复确定性

另起进程重复同一 intact 运行。两次结果均为 population `21,543,617`、motor `80,716`、
summary feeding `2.980 s`、flight `14.190 s`，最终位姿和全部 1,702 个采样点逐项一致。

- samples 的规范化 SHA-256 均为
  `1be3bdb9aee31fb7e1589aaf311642282a4db0df2655fac6888144c673b808d8`；
- 删除七个墙钟字段后的完整报告规范化 SHA-256 均为
  `61137d347a24baf05e1589a8ecc3ec8ecf64df28e0b261292b580f687f14f5e8`。

因此在此固定初态和确定性感官场景中，direct CUDA + 原生 MuJoCo 的 20 秒 population
response、行为状态、接触和轨迹可重复。这里不要求与 CPU-f64 或 Metal 长程逐神经元一致。

## 运行速度

用户确认正式复测时已无外部 GPU 竞争。两次 intact 20 秒端到端分别为 `31.795 s` 和
`29.787 s`，即约 `0.629×` 和 `0.671×` 实时；neural engine 分别为 `8.972 s` 和
`7.393 s`，physics/flight loop 分别为 `20.765 s` 和 `20.474 s`。长程飞行、边界接触
及高空状态令物理侧占比高于早先 1–5 秒短测，进一步确认下一性能阶段应针对原生
MuJoCo/flight loop，而不是继续优化 bridge 或把 Graph 当作主要解法。

## 证据文件

- `outputs/cuda/cns-native-long-intact-20s-2026-09-12.json`，SHA-256
  `58b2812a069f957ccc9c2b2f0f3dd5a5fc71eaaddc2c6ac31901697a55e91759`；
- `outputs/cuda/cns-native-long-intact-repeat-20s-2026-09-12.json`，SHA-256
  `8d93deacf417b16c0cf4b13bf63b591e8d7bb0ad858f257ca05aa58aa0a58a7d`；
- `outputs/cuda/cns-native-long-odor-disconnected-12s-2026-09-12.json`，SHA-256
  `5a356b1fbb6cbe6527ca08a506712ab0b2fa77bcedc9e33647ac315d6c5fda90`；
- `outputs/cuda/cns-native-long-motor-disconnected-12s-2026-09-12.json`，SHA-256
  `c80c507290a60a7670b6bae15330a703ee5dccead0c8db6e606bdca9840f601c`。

## 下一阶段

冻结当前神经/行为基线。下一性能阶段只剖析原生 MuJoCo/flight loop 内部的 wing command、
stabilizer、telemetry、contact/collision 和 `mj_step`，先区分 Rust 控制开销与 MuJoCo
求解开销，再决定是否存在不改变 0.1 ms timestep、身体动力学或行为语义的安全优化。
