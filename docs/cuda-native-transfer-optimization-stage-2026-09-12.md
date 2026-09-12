# CUDA 原生闭环传输优化阶段报告

日期：2026-09-12。环境：WSL2/Linux、NVIDIA GeForce RTX 5080、CUDA Toolkit
13.0、原生 MuJoCo 3.9。本阶段只优化 `CudaEngine` 与 `BrainBodyBridge` 间的设备传输和
临时分配；MaleCNS 数据、f32/CSR/delay ring/refractory、神经执行顺序、sensory/motor
mapping、行为逻辑、模型参数和 MuJoCo 世界均未修改。

## 实现范围

- 稀疏闭环窗口不再在窗口前后读回全部 166,700 个神经元状态，只在设备端收集既有
  probe 的累计 spike count，再读回小数组并在 Rust 侧作 checked delta。
- population total/active 和 mean voltage 改为设备端确定性归约，只读回三个标量。
- external target、sparse event 和 probe 缓冲区改为引擎生命周期内按需扩容并复用，
  消除每个 2 ms 控制窗口中的对应 `cudaMalloc/cudaFree`。
- `cns-check` 增加 brain wall、sensory encoding 和 CUDA engine 分项计时。该计时只用于
  观测，不参与神经或身体状态更新。
- 稠密验证路径仍保留完整状态读回，供 fixture 和完整 CNS 验收使用。

## 正确性与回归

CUDA 的四个 tiny/parity 测试通过。Brian f64 fixture 的首次非零差异仍是 tick 2、
neuron 1 的 conductance：CUDA f32 `13.75`、参考 `13.750000000000002`；最大 voltage
绝对差 `3.907754056342583e-6`，最大 conductance 绝对差
`9.748347178373251e-7`，未出现 spike 分歧。

预注册完整 MaleCNS 验收通过：CUDA 自身重复和分块哈希一致，15 个固定 case 的
population 与 input/relay/motor groups 均与既有参考一致，20 ms spike/state 哈希一致；
历史 MeVP24–DNp10 pathway gate 仍按既有结果失败，没有被后端优化改变。结果为
`outputs/cuda/male_cns_validation_bufferreuse_2026-09-12.json`，SHA-256：
`1d1492d1482423c3701d2e30eaf8b15e6c5ef72008603e8072214cdec820a3b1`。

2 秒原生 MuJoCo 成对门控通过。intact 为 population `2,071,162`、motor `8,345`、
flight `1.822 s`、forward `29.142 mm`；motor-disconnected 为 population `2,072,049`、
motor `8,612`、保持 grounded，最终位姿差 `15.475 mm`。这些值与优化前完全一致。
结果为 `outputs/cuda/cns-native-bufferreuse-world-verification-2026-09-12.json`，
SHA-256：`6ff6cd159e98b2ed8227eb9c446e4fbc7ae009fe6f9e08ebe2067c53a4646e0a`。

库测试共 254 项，其中 251 项通过；其余 3 项因本机没有可选的
`outputs/packs/flywire_v783` 数据而失败。四个 CUDA 测试、MaleCNS 资源测试、原生世界
测试均已通过；这 3 项失败与本次改动无关。

## 性能

固定 0.2 biological seconds、500 Hz、无 settle 的完整 CNS + 原生 MuJoCo 闭环连续
运行三次，内部端到端耗时为 `0.6863/0.6667/0.6794 s`，中位数 `0.6794 s`，约
`0.294×` 实时。分项中位数为：sensory encoding `0.0134 s`、CUDA engine
`0.4137 s`、brain wall `0.4342 s`。三个运行的 population `182,112`、motor `675`
和最终位姿逐项一致。

与持久缓冲区加入前的同一分项构建相比，CUDA engine 从 `0.8238 s` 降至
`0.4137 s`，下降约 `49.8%`；内部端到端从 `1.1820 s` 降至 `0.6794 s`，下降约
`42.5%`。与最初未分项的闭环基线 `0.7375 s` 相比，端到端中位数下降约 `7.9%`。
这说明临时分配曾造成明显回退，但当前闭环仍未达到实时：余下主要成本仍在每个 2 ms
窗口的多次 kernel launch、同步和小传输，而不是 MuJoCo 或 sensory encoding。

## 下一阶段边界

下一性能阶段先测量并减少固定 probe 索引的重复上传和窗口同步；只有在 profile 证明
launch 开销占主导且能够保持严格 tick 顺序时，才评估 CUDA Graph。继续使用相同的
tiny/parity、完整 MaleCNS 固定输入和 2 秒原生 world gate，任何 spike/hash 或世界输出
变化都停止优化并定位。此阶段不开始新世界、翅膀修正或清洁行为。
