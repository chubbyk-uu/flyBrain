# 项目目标与实施规范

制定日期：2026-09-12。本文记录用户确认的目标和边界，是本 fork 后续开发的范围依据。
后续用户指示可更新本规范；计划中的能力不得表述为已经实现。

## 总体目标

基于 [mehrantsi/flyBrain](https://github.com/mehrantsi/flyBrain)，使用同一雄性标本的
MaleCNS v1.0 brain + VNC 数据，运行具有感官输入、神经计算和身体反馈的模拟果蝇。
起始代码版本为 `cdd3a127766ec184e19c4988fe12b6fd2cbc64fd`，开发分支为 `cuda-backend`。

近期目标是确认现有 WebGPU 版本在 Windows/RTX 5080 上的运行路径，并实现 Linux/WSL2
的 `CudaEngine`。现有基线已完成，接下来先诊断性能，再移植独立神经核心。
CUDA 验证之后，用户要求重绘更小的室内世界、改善果蝇外观、检查翼运动，并增加由
神经活动驱动的前足搓擦及头部/复眼清洁行为。各阶段分别记录与验收，不一次性大改。

连接组重建不等于完整复制动物。现有项目使用简化 LIF 神经元及工程化身体接口，不宣称
恢复供体记忆、身份或完整生理状态，也不将可见运动视为生物学行为验证。

## CUDA 移植阶段必须保持的边界

- 保留现有 MaleCNS 数据、神经元 ID/顺序、CSR 拓扑、权重及其来源与哈希校验。
- 保留 `BrainBodyBridge` 的感官编码、probe 选择、滤波、sensory/motor mapping 和解码算法。
- 保留 MuJoCo 3.9.0 世界、NeuroMechFly 身体、碰撞、执行器、步态、翼运动、稳定控制和行为仲裁。
- 神经与物理步长保持 `0.1 ms`，默认每窗口 20 步，以 500 Hz 进行因果脑体交换。
- 仅替换 GPU 神经计算后端，允许必要的平台编译、动态链接、类型选择及验证入口适配。
- 保留 Metal 和浏览器 WebGPU 路径；不引入跨标本拼接、虚构连接或新的运动策略。
- 不通过减少神经元、删边、降采样或改变模型参数换取性能。
- 环境安装仅使用本项目独立 Conda 环境 `flybrain`，不再修改其他项目的共享环境。

上述世界、身体与映射冻结约束适用于原始基线及 CUDA 移植/对照阶段。
后续场景、翼运动和清洁阶段允许其各自所需的视觉、身体控制或感官/运动接口扩展，
但必须与原始基线分开验证，不覆盖基线资产和实验记录。MaleCNS 原始数组与身份校验
继续保持不变；接口扩展须单独记录证据、版本和假设，不能以修改连接组代替通路验证。

## 数据与科学边界

上游导入模型包含 166,700 个神经元、24,469,412 条有符号有向边和 120,260,398 个
解剖接触。它不是原始连接表的全部分割片段：节点按 `superclass != null` 选择，
未知或未支持的递质效应对应的出边被显式省略；这些现有选择规则不得在移植中改变。

现有视觉通道在 MaleCNS 模式使用运动/接近代理输入，不是完整视网膜转导；食物搜索、
步态与飞行控制仍包含工程解码器。具体边界见
[CNS embodiment](cns-embodiment.md)、[odor guidance](cns-odor-guidance.md) 和
[pathway assay](male-cns-pathway.md)。原始数据来源见
[MaleCNS 官方下载页](https://male-cns.janelia.org/download/)。

现有舔糖、飞行、行走属于“神经活动驱动、工程控制器执行”的混合系统：

| 动作 | 神经活动参与 | 工程执行部分 |
|---|---|---|
| 舔糖/口器伸展 | 味觉输入经 CNS 传播，实际 MN9 脉冲参与伸展控制 | 接触到刺激频率、脉冲到关节角度、进食保持和离开时序 |
| 飞行 | 翼动力/转向运动神经元读出及相关活动门控 | 拍翼轨迹、姿态/速度稳定、升降和避障控制 |
| 行走 | 腿部运动神经元读出控制步态驱动/节奏 | 实测步态对应的关节轨迹及身体协调 |

这不等于每一下拍翼或每个关节轨迹都由连接组直接产生。上游报告的运动断连实验
支持动作对神经读出的依赖；本机阶段 1 尚未复验这些断连实验，不能当作本机实测结论。

## 运行路线

| 路线 | 组成 | 当前状态 |
|---|---|---|
| Windows 浏览器 | Rust/MuJoCo WASM + WebGPU 神经计算 + 浏览器显示 | Windows Chrome/RTX 5080 完整 CNS 基线已通过，Edge 未单独测量 |
| macOS 原生 | Rust + Metal + MuJoCo/GLFW | 保留上游路径 |
| Linux/WSL2 原生 | 原生 Rust + CUDA + 原生 MuJoCo + 同一世界/身体资产 | 待实现及验收 |

浏览器优先使用 Windows 侧 Chrome/Edge；WSL 可只运行本地静态服务器。
浏览器版不需要 CUDA Toolkit。源码重建才需要上游固定的 Emscripten 4.0.10 和
MuJoCo 3.9.0 源码，详见 [browser runtime](browser.md)。

CUDA 路线不得把 MuJoCo 留在浏览器或 WASM 中。WebGPU/WASM 路线只作为已校验基线、
演示和跨平台参考保留；正式 WSL/Linux 闭环必须由原生 Rust 进程同时承载 `CudaEngine`、
原生 MuJoCo 和后续显示层。第一版 CUDA 核心仍按隔离原则不接世界，核心验收完成后才
接入原生 MuJoCo，且继续使用相同 MJCF、身体资产、世界状态与 `BrainBodyBridge` 语义。

仓库提交了 WASM 和运行资源，但 `.gitignore` 排除了 `outputs/packs/`。
新克隆必须先恢复 `outputs/packs/male_cns_v1`；本机已恢复并校验。
不能把 `npm start` 成功当作完整 CNS 成功。
优先获取与现有 I/O 哈希一致的作者数据包；线上大数组分块需顺序重组并校验原始完整哈希。
若改从官方 Feather 重建，必须使用既有导入器和规则，验证四个数组与 I/O 绑定一致。
不得重新生成 I/O 或更换数据来绕过不匹配。

## 最小 CUDA 设计

首步只新增独立 `CudaEngine` 与测试/诊断入口，不接 MuJoCo/world，不修改
`BrainBodyBridge` 的后端选择。后续接入世界时才沿用现成的 `NeuralEngine` 类型别名，
添加 Linux/CUDA 分支，不为此引入通用后端框架。核心兼容构造参数、`run_window_sparse()`、
窗口 `elapsed`/`spike_count_deltas` 和设备、内存、群体统计接口；离线验证需要
`run_schedule()`、`run_recorded()` 等对应能力。

逐项移植 `flybrain.metal` 的七个内核：`decay_threshold`、`propagate_csr`、
`decay_threshold_propagate_delayed`、`reset_decay_threshold`、`apply_external`、
`apply_external_sparse`、`reset_store`。保持 f32 状态、i16 权重、i32 原子累加、
严格阈值、延迟环、不应期、静默源和窗口边界顺序。特别核对零延迟和非零延迟分支。

CSR 和神经状态常驻设备显存。使用有序 CUDA stream 批量提交窗口，上传稀疏事件并
集中读回 probe；保留遥测语义。不得照搬 Metal 共享内存的主机直接解引用方式。
首版以一致性为主，不同时引入新的稀疏传播算法。是否使用 NVRTC、CUDA Graph 或额外
归约内核，依据接口验证与测量决定，不作为首版前置重构要求。

## 实施顺序

1. 原始基线（已完成）：独立环境、MaleCNS/资源校验、Windows Chrome/RTX 5080
   WebGPU QA、20 秒带渲染完整 CNS、重置检查；见 [实测报告](baseline-rtx5080-windows.md)。
2. 性能诊断（已完成）：GPU timestamps、无观察者和 world-only 对照表明，WebGPU
   compute pass 平均约 `0.506 ms/窗口`，但同步式浏览器 neural-engine 路径约
   `4.000 ms/窗口`；world-only 的 WASM 物理仍只有 `0.301×`。详见实测报告。
3. 独立 CUDA 神经核心（已完成）：Rust/CUDA 核心、tiny fixture、自身确定性、
   chunk/sparse/split-window parity 已通过；完整 MaleCNS 的 15 个固定输入 case、关键群体
   和既有实验门控也已通过预注册验收。见 [CUDA 核心阶段报告](cuda-engine-stage-2026-09-12.md)
   与 [完整 CNS CUDA 报告](cuda-full-cns-stage-2026-09-12.md)。
4. 原生世界接入与实验回归（进行中）：保持原世界/映射，CUDA 已接入原生 MuJoCo，
   不经过浏览器/WASM；2 秒 `cns-check` 成对门控、无感官输入、运动/嗅觉断连和
   WSLg viewer 已通过。bridge 已改为设备端 probe/telemetry 小读回并复用 CUDA 临时
   缓冲区；固定 probe 也已跨窗口缓存，50 ms profile 的 CUDA copy 调用从 154 次降至
   92 次。固定 0.2 秒闭环曾测得约 `0.294×` 实时，但 1 秒复测仅约 `0.184×`，证明
   低活动短测不能代表持续性能。CUDA 已默认使用保持原 i32 arrival 和 tick 顺序的
   256-edge chunked CSR propagation；1 秒复测的 neural engine 达到约 `2.40×` 实时，
   完整闭环达到约 `0.800×`，且完整 CNS 与 world gate 均保持一致。重新 profile 后，
   1 秒窗口中 physics/flight loop 中位数约 `0.754 s`、brain wall 约 `0.513 s`，其余
   bridge/控制约 `0.009 s`；下一步只实验保持 tick 顺序的 CUDA Graph，再运行较长
   取食/嗅觉门控。见
   [原生接入报告](native-cuda-mujoco-stage-2026-09-12.md)与
   [CUDA 闭环传输优化报告](cuda-native-transfer-optimization-stage-2026-09-12.md)、
   [probe 缓存与瓶颈复测报告](cuda-native-probe-cache-stage-2026-09-12.md)、
   [chunked CSR propagation 报告](cuda-chunked-propagation-stage-2026-09-12.md)、
   [原生闭环组件分项报告](native-closed-loop-component-profile-2026-09-12.md)。
5. 新世界与果蝇呈现（后续）：更小的室内场景、外观改进、翼运动诊断与相应修正。
6. 神经驱动清洁行为（后续）：前足相互搓擦、头部和复眼清洁，验证通路与身体协调。

每阶段完成后先汇报结果，再继续下一阶段。性能均分别报告加载、神经计算/传输、
桥接、物理与显示开销；不承诺 RTX 5080 必达实时，不把上游 M3 Max 测量外推成本机结果。

## CUDA 验收标准

不要求 CUDA 与 CPU-f64 长时间逐神经元完全一致。验收分三层：

1. CUDA 自身确定性：同设备、同构建、同初态和同固定输入重复运行结果一致；
   改变窗口划分不得改变结果。墙钟驱动的视网膜输入不是固定输入，不用于直接证明
   长期闭环逐位确定性；如需对比，先固定并回放感官事件。
2. 短程正确性：tiny fixture 的 spike、延迟环、不应期、严格阈值、抑制、静默源、
   执行顺序和稠密/稀疏输入等价正确，覆盖跨窗口及高扇出情况。
   f32 状态使用预先定义的容差。报告首次非零数值差异、首次超容差和首次 spike
   时序分歧的 tick/神经元/状态；未发生分歧时说明测试范围，不把正常舍入差异判为失败。
3. 完整 CNS 实验一致性：固定刺激下比较 population response，并检查关键感官、DN、
   运动群体的响应与已有门控/断连效应，不能只比较全脑平均值。
   指标、观察窗口、对照条件和容差在验收前确定。独立神经实验与接入世界后的行为门控
   分开执行；后者使用同一原始世界和映射。

CPU-f64 用于短程语义参考和差异诊断，既有 f32/FMA CPU 诊断用于区分精度效应与
调度实现错误。长程 f32/f64 偏离本身不判定 CUDA 失败。

数值验证必须保留上游已知失败：100 ms MaleCNS CPU-f64/Metal 的严格 `0.001 mV`
最终状态门槛失败，MeVP24–DNp10 候选通路的生物学验收也失败。
这两项仍作为历史失败保留，不把严格长程 f64 逐神经元状态门槛当作 CUDA 的新验收要求，
也不要求 CUDA 将已失败的生物学假说“跑成通过”。不能仅凭累计计数一致宣称逐步一致，
不得事后放宽原门槛、修改历史结论或调行为参数掩盖移植错误。

## 后续场景、外观与翼运动需求

用户指定新世界比现有场景更小，仍为室内，包含茶几、糖果，以及带花的盆栽植物，
提供可落脚、探索、清洁和取食的位置。具体房间尺寸、物体布局和美术风格尚未确定。
同时改善果蝇外观；新场景作为独立配置/资产版本保留，原始基线仍能重现。

气味与接触味觉分开建模：纯糖不作为远距离气味源；带香味的糖果及花朵可作为后续
嗅觉刺激来源。具体气味身份、浓度和感官路由待查证与明确，不因摆放花朵就假定
MaleCNS 已具备相应花香识别，也不让控制器直接读取食物坐标来替代感官导航。

用户认为现有翅膀运动看起来不对，这是待诊断的问题，尚未证实运动算法有误。
先区分显示帧率/采样混叠、翼关节坐标与轨迹、拍翼相位及物理飞行控制问题。
视觉修正与真实运动/控制修正分开验证；如涉及后者，记录关节运动、支撑/升力及稳定性
回归，不能在 CUDA 对照中同时改变拍翼算法。

## 神经驱动搓脚与头眼清洁

用户明确要求按现有动作的混合驱动标准实现自主清洁：前足相互搓擦，以及前足清洁
头部和复眼区域，参照真实果蝇的动作与协调性。不能只添加空闲计时器触发的动画。

- 感官输入进入 MaleCNS，相关神经活动触发或调节清洁动作；具体感官刺激与状态条件待研究。
- 身体控制器执行前足轨迹、接触与支撑协调，并处理与行走、飞行、进食的动作冲突。
- 查证同一 MaleCNS 标本内候选神经元、到前运动/运动群体的通路及对应读出，
  不能把候选细胞存在等同于通路已经验证，也不能复制其他标本 ID 或虚构连接。
- 通过完整通路、相关神经元静默/通路断连、运动读出断连和适当刺激对照，验证动作
  对神经活动的依赖；测量足间及足与头/眼接触、轨迹、顺序和身体支撑稳定性。
- 现有 grooming 是工程姿态诊断，不能改名后称为神经行为。外观相似不等于生物学复现；
  “清洁”首步指接触/动作，若要声称去除污染，需另定义污染状态和去除指标。

背景边界见 [上游 neural grooming 文档](neural-grooming.md)。当前尚无本项目新增
清洁通路、动作控制器或验证结果；后续实现应明确哪些是证据支持、哪些是工程假设。

## 预计改动文件

以下为 CUDA 及原世界接入的计划清单，不代表已经修改。首步只涉及核心、构建和独立
验证入口；桥接选择、MuJoCo 依赖与 GLFW 链接留待原世界接入阶段。
新世界/外观/清洁阶段的具体文件清单在相应设计时补充。

| 文件 | 改动目的 |
|---|---|
| 新增 `rust/src/cuda_engine.rs` | CUDA 状态、调度、传输、读回和兼容 API |
| 新增 `rust/cuda/flybrain_cuda.cu` | 神经内核与 CUDA runtime 边界 |
| `Cargo.toml`、`build.rs` | 可选 CUDA feature 与 `nvcc` 构建；Linux MuJoCo 依赖留待后续 |
| `rust/src/lib.rs` | 模块的平台条件编译 |
| `rust/src/brain_bridge.rs` | 仅后端 import/type alias |
| `rust/src/main.rs`、`output.rs`、`bin/cns_numerics.rs` | 消除验证入口硬编码的 Metal 类型依赖 |
| `rust/src/live_viewer.rs` | GLFW 平台链接名称 |
| `.cargo/config.toml`、`tools/setup_mujoco_runtime.py` | Linux 动态库、搜索路径和运行库准备 |
| 新增 CUDA 验证测试和运行说明 | 可复现验证与性能记录 |

编译如暴露额外平台耦合，可补充必要的条件编译或链接适配，但不得改变世界和桥接算法。
数据、I/O JSON、`metal_engine.rs`、`browser_engine.rs` 及现有身体资产不属于算法修改范围。

## 状态管理

当前状态：阶段 1、性能诊断、独立 CUDA 核心及完整 CNS 固定回放验收已完成，见
[Windows/RTX 5080 基线](baseline-rtx5080-windows.md)和
[CUDA 核心阶段报告](cuda-engine-stage-2026-09-12.md)、
[完整 CNS CUDA 报告](cuda-full-cns-stage-2026-09-12.md)。
2026-09-12 的后续顺序为：CUDA bridge 读回优化及长程原生世界回归 →
新室内世界/果蝇外观/翼运动 → 神经驱动搓脚与头眼清洁。
当前 CUDA 已接 `BrainBodyBridge` 和原生 MuJoCo/world；2 秒成对门控通过，且没有修改
数据、映射、行为或模型参数。隔离 intact 神经回放初测平均约 `1.23×` 实时，但首版
兼容 bridge 闭环只有约 `0.271×`，全状态读回优化尚未完成。后续仍只使用原生 MuJoCo，
不使用浏览器/WASM 物理。
本机检查与环境恢复记录见 [环境检查记录](environment-check-2026-09-12.md)。
历史上游测试结果以原文档为准，不标为本机通过。后续阶段结果单独记录日期、版本、
输入哈希、命令及通过/失败/未执行状态，再更新本规范的进度描述。
