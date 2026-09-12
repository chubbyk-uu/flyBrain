# CUDA Graph 实验报告

日期：2026-09-12。环境：WSL2/Linux、NVIDIA GeForce RTX 5080、CUDA Toolkit 13.0、
默认 `chunked-256` propagation、原生 MuJoCo 3.9。本阶段没有修改 MaleCNS 数据、
sensory/motor mapping、行为逻辑、模型参数、物理步长或世界。

## 实现边界

`FLYBRAIN_CUDA_EXECUTION=graph` 为稀疏闭环窗口启用实验 CUDA Graph；未设置或显式设为
`direct` 时继续使用原稳定路径。Graph 只捕获窗口内按原顺序排列的神经 kernel：
decay/threshold/propagation、external input、reset/store。感官事件上传、probe gather 和
读回仍在图外。

稀疏 step offsets 新增长驻设备缓冲；每个 external kernel 从设备 offsets 读取该 tick
的事件范围。Graph 按窗口步数和 delay-ring 起始槽缓存。当前 18-tick delay ring 配合
20-tick 控制窗口会轮转 9 个起始槽，因此不能只缓存一个图。稀疏事件缓冲扩容时使相关
图失效；每个图记录已捕获的单 tick 最大 external event 数。`direct` 是完整回退路径。

CUDA stream capture 与同进程另一个线程的 `cudaDeviceSynchronize` 不可并发，因此 Rust
FFI 调用增加进程内互斥。单实例闭环的锁不竞争，不改变 GPU 执行顺序。

## 正确性

- direct 与 Graph 各自通过 4 项 CUDA tiny/parity 测试；覆盖 spike、f32 短程容差、
  delay ring、signed edge、silencing、refractory、dense/sparse/split window 和空窗口。
- 两种路径的 tiny fixture 首个非零差异仍为 tick 2、neuron 1 的 conductance：CUDA
  `13.75`，Brian f64 `13.750000000000002`；最大 voltage 绝对差约 `3.91e-6 mV`、最大
  conductance 绝对差约 `9.75e-7 mV`，没有 spike 分歧。
- 5 秒 direct/Graph 各三次完整 CNS 闭环均为 population `5,285,414`、motor `21,126`，
  最终位姿逐项一致。
- Graph 的 2 秒完整 CNS + 原生 MuJoCo 成对门控通过。intact 为 population
  `2,071,162`、motor `8,345`、飞行 `1.822 s`、前进 `29.142 mm`；motor-disconnected
  为 population `2,072,049`、motor `8,612`，保持 grounded。

最终门控文件：
`outputs/cuda/cns-native-graph-final-world-verification-2026-09-12.json`，SHA-256：
`e2258041d93f2716a76074a9cff3265f8a84259c38b98443ba108391a110c71e`。

## 性能结果

GPU 未出现后续竞争负载时，5 biological seconds、500 Hz、无 settle 的三次中位数：

| 模式 | neural engine | brain wall | physics | endpoint | 实时倍率 |
|---|---:|---:|---:|---:|---:|
| direct | `2.0919 s` | `2.5405 s` | `3.4338 s` | `6.0395 s` | `0.828×` |
| Graph | `2.0042 s` | `2.4572 s` | `3.5377 s` | `6.0672 s` | `0.824×` |

Graph 令 neural engine 中位数快约 `4.2%`，但没有改善完整闭环中位数，也未达到预设的
`0.95×` headless 目标。因此不把 Graph 晋升为默认值。

Nsight Systems 的 0.2 秒诊断中，direct 记录 `5,789` 次 `cudaLaunchKernel`；最终 Graph
路径记录 `100` 次 `cudaGraphLaunch`，capture 期间有 `1,986` 次 kernel launch 和
`31` 次 instantiate。WSL 下该次报告没有 GPU kernel timing，API 计数只证明提交结构
发生变化，不用于替代上述端到端墙钟。

随后检测到 `ruby3.2`、`rviz2` 等外部进程共同占用约 10 GB 显存且 GPU 有持续负载；
同一 direct 1 秒测试也从约 `0.42 s` neural engine 退化到 `0.65–1.06 s`。该时段结果
不纳入性能结论，也没有终止外部进程。

5 秒原始证据为
`outputs/cuda/cns-native-graph-{direct,graph}-5s-trial{1,2,3}-2026-09-12.json`。

## 结论与下一步

CUDA Graph 的行为一致，但对当前闭环只有小幅神经侧收益，完整系统仍由串行相加的
physics/flight loop 和 brain 共同限制。保持 `direct` 默认和 Graph 可选回退后，下一步
先完成 20 秒稳定性、取食/舔糖、嗅觉及断连门控；性能工作转向原生 MuJoCo/flight loop
的安全剖析，不改变 0.1 ms 物理步长来制造表面加速。
