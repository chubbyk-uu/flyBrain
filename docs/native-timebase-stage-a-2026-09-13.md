# 原生闭环时间基准分离（阶段 A）

日期：2026-09-13。状态：实现及 20 秒完整 CNS 验收完成；0.2 ms 已被接受为原生实时默认值。

## 实现

`SimulationStepper` 现在显式保存控制周期、神经 dt、物理 dt 及各自的整数步数。
500 Hz 控制窗仍为 2 ms；默认运行保持 20 个 0.1 ms MaleCNS tick 和 20 个
0.1 ms MuJoCo tick。`cns-check --physics-dt-ms 0.2` 只将物理侧改为每窗 10 步，
神经侧仍运行 20 步。调度器要求物理 dt、神经 dt 和控制窗构成可验证的整数比例，
不按墙钟丢 tick，也不允许分窗积累小数时间误差。

原 `SimulationStepper::new_with_parameters` 和未传新参数的命令保持参考配置。
`cns-check` 报告的 `timebase` 字段记录实际 dt 和步数。MaleCNS 数据、CUDA 神经参数、
CSR、delay ring、refractory、BrainBodyBridge、感觉/运动映射、行为逻辑和资产均未改变。

## 验证

构建命令为：

```bash
cargo build --release --bin flybrain-world --features cuda
```

性能运行使用同一个 optimized release 二进制，SHA-256 为
`a5b5cf3f2c0bca9ec71da7c234013fd194537cfe3ed04021fba15621f9b8eae8`，并设置
`FLYBRAIN_CUDA_EXECUTION=graph`、`FLYBRAIN_PROFILE_PHYSICS=1`。两次 10 生物秒
headless 命令除候选增加 `--physics-dt-ms 0.2` 外相同。

| 指标 | 0.1 ms 参考 | 0.2 ms 物理候选 |
|---|---:|---:|
| 神经步/2 ms 窗 | 20 | 20 |
| 物理步/2 ms 窗 | 20 | 10 |
| 墙钟时间 | 16.786 s | 10.441 s |
| 实时倍率 | 0.596× | 0.958× |
| brain wall | 5.071 s | 4.671 s |
| physics wall | 11.591 s | 5.661 s |
| `mj_step` wall | 11.389 s | 5.558 s |
| population spikes | 10,840,600 | 10,786,843 |
| motor output spikes | 40,422 | 41,155 |
| flight seconds | 5.156 s | 4.868 s |
| feeding seconds | 2.980 s | 0.662 s |

总吞吐提升约 1.61×，物理分项约快 2.05×。候选完成起飞、巡航、降落、持续地面接触和
取食，没有非有限状态、自动重置或明显接触爆炸。两种物理 dt 从第一物理窗起就允许产生
不同身体轨迹及感觉输入，因此不要求完整闭环 spike 或轨迹逐值相等；上表 spike 差异不能
单独解释为神经核心不一致。神经核心本身没有改动。

参考报告为
`outputs/cuda/phase-a-timebase-reference-0p1ms-10s-2026-09-13.json`
（SHA-256 `d600055e9680ea56f41ee71db26128ba9a072b79862e80f182339a62263b229c`）；
候选报告为
`outputs/cuda/phase-a-timebase-candidate-0p2ms-10s-2026-09-13.json`
（SHA-256 `e76a49ab7b7b02013ed7824c404a3a3a6f43b4b17ae08ff2fe68bf997c488d99`）。

单元测试另验证：显式 0.1 ms 与原默认的短程 qpos/qvel/control 完全相同；0.2 ms
连续控制窗的神经/物理时间一致；不支持的非整数比例会被拒绝。

## 结论与边界

补充的 20 秒完整 CNS 运行耗时 18.771 秒，约 `1.065×` 实时；完成约 2.998 秒连续
取食，随后进入 POST-MEAL 并重新起飞离开，没有非有限状态或自动重置。结果为
`outputs/cuda/phase-a-timebase-candidate-0p2ms-long-20s-2026-09-13.json`，SHA-256
`88f63f754a35c7c0cf5c02ac7bc2d27340eea17d294141e82791e217712d5b1f`。

阶段 A 因此验收通过，0.2 ms 被接受为原生实时路线默认物理步长；MaleCNS 神经仍为
0.1 ms。显式 0.1 ms 保留为历史参考和回归模式。当前不再以制作轻量身体作为下一阶段，
优先优化 viewer FPS。长期飞行范围及探索行为仍需后续混合行为阶段处理，本结论不把
20 秒取食门控扩展成数分钟自主行为验收。
