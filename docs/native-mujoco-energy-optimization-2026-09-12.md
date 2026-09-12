# MuJoCo energy 统计优化报告

日期：2026-09-12。上一阶段确认原生物理的 95.66% 位于 MuJoCo `mj_step`。本阶段仅关闭
MJCF 中原本启用、但 flyBrain 运行时没有读取的 energy 统计；没有修改 0.1 ms timestep、
求解器、碰撞、身体/世界资产、控制值、MaleCNS、bridge、映射、行为逻辑或模型参数。

实现保持 `assets/neuromechfly/fly.xml` 不变，在加载模型后默认清除 MuJoCo energy enable
flag；诊断时可用 `FLYBRAIN_MUJOCO_ENERGY=1` 恢复原设置。`cns-check` 的
`summary.physics_profile.mujoco_energy_enabled` 会记录实际状态。

## 性能 A/B

全部测试为 release、headless、完整 CNS、3 biological seconds、500 Hz control、0.1 ms
physics timestep。为抵消热身和调频顺序偏差，前三组按 energy on→off，后三组反向：

| trial | energy on `mj_step` | energy off `mj_step` | off 改善 |
| ---: | ---: | ---: | ---: |
| 1 | 1.975 s | 1.934 s | 2.09% |
| 2 | 2.122 s | 1.901 s | 10.40% |
| 3 | 1.928 s | 1.861 s | 3.50% |
| 4 | 1.907 s | 1.859 s | 2.49% |
| 5 | 1.922 s | 1.874 s | 2.50% |
| 6 | 1.902 s | 1.868 s | 1.83% |
| 中位数 | 1.925 s | 1.871 s | 2.83% |

六组方向一致。trial 2 有明显系统抖动，因此采用全部六组的中位数，不使用最大值描述收益。
一秒 paired A/B 在删除 wall-clock 和 profile 元数据后，85 个采样点及全部神经、行为、
接触和轨迹输出逐字节一致。

## 完整 CNS 验收

默认 energy off 后重新运行 20 秒 intact、12 秒嗅觉诱发输入断连和 12 秒 motor 断连。
既有 `verify_cns_odor_guidance.py` 的 21/21 检查全部通过：

- intact population `21,543,617`、motor `80,716`、连续严格取食 `2.964 s`、
  `11.018 s` 餐后离开、飞行 `14.190 s`；
- 嗅觉断连 population `12,580,771`、motor `51,342`，无 guidance/feeding；
- motor 断连 population `12,571,427`、motor probe `51,672`，无飞行、伸吻或取食。

20 秒 intact 与此前 energy on 长程基线在删除墙钟、新增 profile 元数据、二进制 SHA 及由其
派生的初态 SHA 后完整 JSON 逐字节一致。新 intact 的 physics wall time 为 `19.518 s`；
旧两次为 `20.765 s`、`20.474 s`。跨批次完整 wall-clock 还受 CUDA 调频影响，因此正式
收益采用同批次交替 A/B 的 MuJoCo 中位数 `2.83%`，不声称整体闭环固定提升同一比例。

证据：

- `outputs/cuda/mujoco-energy-ab-summary-2026-09-12.json`；
- `outputs/cuda/cns-native-energy-default-intact-20s-2026-09-12.json`；
- `outputs/cuda/cns-native-energy-default-odor-disconnected-12s-2026-09-12.json`；
- `outputs/cuda/cns-native-energy-default-motor-disconnected-12s-2026-09-12.json`；
- `outputs/cuda/cns-native-energy-default-odor-guidance-verification-2026-09-12.json`。

定向严格 Clippy、`world_sim` 27/27 tests 和长程 21/21 行为门控均通过。

## 下一步

下一候选应继续在 `mj_step` 内做单变量实验。求解器或碰撞参数可能影响接触轨迹，风险显著
高于 energy 统计，必须保持 opt-in，先做接触/飞行 A/B 和长程行为验收，不能因速度直接采用。
