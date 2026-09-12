# 原生 MuJoCo 物理分项报告

日期：2026-09-12。环境：WSL2、RTX 5080、release CUDA 后端、原生 MuJoCo 3.9.0。
本阶段只加入可选计时，不修改 MaleCNS 数据、`BrainBodyBridge`、感官/运动映射、行为逻辑、
模型参数或物理执行顺序。所有基准均使用无 viewer、无渲染的 headless `cns-check`。

## 方法

设置 `FLYBRAIN_PROFILE_PHYSICS=1` 后，每个 0.1 ms 物理子步分别累计：飞行命令与稳定器、
飞行施力、MuJoCo `mj_step`、有限值校验、步后流体力读取和遥测归约。环境变量未设置时仍走
原来的 `FlightRuntime::advance` 路径；只多一个窗口内分支，不执行细分计时。

先用相同初态运行一对 1 秒 profile off/on 对照。剔除 wall-clock 和新增 profile 字段后，
两份报告的全部非计时内容逐字节一致，包括 85 个记录样本、轨迹、spike 计数和行为状态。

随后在 GPU 无外部负载时连续运行三次 5 秒 headless 完整 CNS。三次轨迹和神经输出一致；
wall-clock 为 6.023、5.991、5.889 秒，离散为 2.24%。中位数如下：

| 项目 | 5 秒模拟的 wall time | 占物理时间 |
| --- | ---: | ---: |
| 原生 MuJoCo `mj_step` | 3.192 s | 95.66% |
| 飞行施力/控制写入 | 0.116 s | 3.48% |
| 命令与稳定器 | 0.012 s | 0.36% |
| 状态有限值校验 | 0.006 s | 0.19% |
| 步后读取、遥测和未分类 | 0.012 s | 0.35% |
| 物理合计 | 3.337 s | 100% |

完整闭环中位数为 `0.835×` 实时；物理占总 wall-clock 55.70%，CUDA neural engine 为
2.114 秒。结论是当前首要热点已经位于 MuJoCo `mj_step` 内部，继续优化 Rust 桥接、遥测或
每步有限值校验的收益上限很小。

下一阶段只做 MuJoCo 配置的单变量 A/B。首先检查当前启用但运行时未读取的 energy 统计，
以及求解器/碰撞选项；任何默认值变更都必须通过短程逐样本 parity、重复确定性和完整 CNS
population/行为门控，不能仅凭速度采用。物理 timestep、身体资产、碰撞世界和控制语义保持不变。

GUI 不属于本报告。原生 viewer 还会增加 OpenGL 绘制、窗口合成与可能的同步/帧率限制，
在 WSLg 下一般会比 headless 更慢；后续应把 viewer FPS 与 headless simulation throughput
分开报告，不能用 GUI wall-clock 反推神经或物理核心速度。

## 证据

- `cns-native-physics-profile-off-1s-2026-09-12.json`：
  `047c70dc2979f16d420796be33c95c41182c5be5c971eba6ee8201498ed1685b`
- `cns-native-physics-profile-on-1s-2026-09-12.json`：
  `cc283a0963cc54c2fc42a436445b391d36e5e93a11661e0121c6355e9a8b1440`
- `cns-native-physics-profile-5s-trial1-2026-09-12.json`：
  `63bb263f12bf4a86758f3804ba3cfdd011cd723b6df7614e89ef5c062854bc54`
- `cns-native-physics-profile-5s-trial2-2026-09-12.json`：
  `4a109acec76ccfe7771c799b3e22c3eb9639c175aecef46dfb7f5b1515c40400`
- `cns-native-physics-profile-5s-trial3-2026-09-12.json`：
  `32071669f0e5459969cbc78b0abb727f7b24acf49f3e0c1f38b78439f94c9d6f`

验证：`cargo check --features cuda --bin flybrain-world`、定向严格 Clippy，以及
`cargo test --lib --features cuda world_sim::tests`（27/27）均通过。
