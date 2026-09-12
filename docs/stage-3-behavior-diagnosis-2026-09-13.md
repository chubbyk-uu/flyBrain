# 阶段 3：300 秒既有行为诊断结果

状态：**通过诊断门控**。本阶段只增加离线分析工具和测试，没有修改生产行为、模型或参数。
“通过”只表示问题已经被可复核地定位，不表示行为已经修好。

## 固定输入与产物

- 基线提交：`7ef9e6aef277eae7deb9cea3873cbd283898c9c1`
- 场景：`small-room-v1`，scene SHA-256 `d763b7af...34810`，habitat SHA-256
  `49b86d2d...5821`
- release binary SHA-256：`2c44432e...efd8`
- 原始记录：`/tmp/flybrain-stage3-cns-300s.json`，SHA-256 `6a690300...0064`
- 分析结果：`/tmp/flybrain-stage3-analysis.json`，SHA-256 `f1b60d22...1df3`
- 命令：
  `target/release/flybrain-world cns-check --scene small-room-v1 --duration-seconds 300 --output /tmp/flybrain-stage3-cns-300s.json`
- 分析：
  `conda run -n flybrain python tools/analyze_behavior_trace.py /tmp/flybrain-stage3-cns-300s.json --output /tmp/flybrain-stage3-analysis.json`

## 连续性与性能

- 连续仿真 `300.000000008 s`，墙钟 `231.019 s`，平均 `1.299× realtime`。
- 记录 29,004 个样本，时间范围 `0.002–299.996 s`；中位间隔 `10 ms`，最大间隔
  `12 ms`，重复/倒退时间戳 0。
- CUDA 神经引擎 `98.953 s`，神经总计 `123.181 s`，MuJoCo physics `105.101 s`。
- 全程无非有限值、自动 reset 或已报告 physics warning。

## 行为分割

| mode | 驻留时间 | bout | 最长 bout |
|---|---:|---:|---:|
| flight/GROUNDED | 0.178 s | 1 | 0.178 s |
| flight/TAKEOFF | 0.082 s | 1 | 0.082 s |
| flight/CRUISE | 299.744 s | 1 | 299.744 s |
| behavior/EXPLORE | 300.004 s | 1 | 300.004 s |
| foraging/SEARCH | 300.004 s | 1 | 300.004 s |

注：末样本使用中位采样周期作为尾区间，因此离线驻留和为 `300.004 s`，相对仿真终点仅
4 ms 量化差。分析器使用每个样本的实际间隔，避免偶发 12 ms 样本造成约 10 秒少计。

- 采样轨迹长度 `36,552.80 mm`，净平面位移 `155.92 mm`。
- 访问 74/660 个 20 mm 平面网格，占用率 `11.21%`。
- 同号 steering 最长 `0.560 s`；最长无支撑飞行 `1.520 s`。
- 接触 bout 3,503；taste bout 0；feeding bout 0。
- 29 个完整 10 秒窗口中：曲率/闭合转向 24、空间栅格循环 29、往返自相关 29。
  三种判据均在读取长跑结果前固定，且合成圆周、直线和周期往返 fixture 已通过。

## “来回飞”的因果链

遥测显示 CNS flight drive 几乎持续饱和：均值 `0.8857`、最大 `0.9054`，29,004 个样本中
29,001 个非零；MN9 rate 均值 `85.44 Hz`。这使起飞 dwell 满足后从 `GROUNDED` 进入
`TAKEOFF/CRUISE`（`flight_behavior.rs:308-329`）。完整 CNS 模式下，单纯表面接触不能令
`CRUISE` 回到 `GROUNDED`，该兜底明确要求 `!brain_enabled`；空中只有
`landing_request` 能进入 `LANDING`（`flight_behavior.rs:331-348`）。

landing DN 并非完全静默（均值 `0.01390`、最大 `0.18665`），但 foraging 仲裁要求它同时具有
近食物/双侧气味平衡或表面接触语境（`foraging.rs:200-207,260-274`）。本次左右气味虽为有限小值
（均值约 `0.000226 ppm`），从未形成 approach/landing 语境；`foraging_mode` 始终 SEARCH，
因而 `landing_request` 始终不足以结束飞行。

运动随后到达房间边界。29,004 个样本中 collision reflex active 22,916 个；运行位置 x 范围
`[-156.971,156.979] mm`，而 y 仅 `[-31.424,44.361] mm`。`world_sim.rs:995-1071` 在障碍/
墙逃逸期间用安全方向覆盖平面速度，再恢复 CNS steering；没有慢时标探索航向或疲劳退出条件，
所以同一东西向通道被反复使用，29/29 窗口呈高自相关往返。视频中的主要故障因此是
**持续 flight drive → 无着陆语境 → 墙逃逸覆盖 → 回到原航向 → 再次撞墙**，不是 viewer 插值。

已排除：持续同号 steering（最长仅 0.56 s）、边界 avoidance 标量自身锁死（记录值全为 0；
活跃的是 collision/wall escape 路径）、味觉/feeding 状态抖动（两者事件均为 0）。曲率判据在
24 个窗口触发，但结合 x/y 范围和 29/29 往返自相关，主导几何是墙间折返，并非单点圆圈。

## 门控验证

- `conda run -n flybrain pytest -q tests/test_behavior_trace.py`：3 passed。
- 阶段 2 与阶段 3 原始记录各取 `time_seconds <= 1.0` 的完整 samples，规范化 JSON SHA-256
  均为 `58ff969f...d3cf`；固定输入短程生产行为逐样本一致。
- 本阶段 git diff 仅包含 `tools/analyze_behavior_trace.py`、对应测试和本报告。

结论：阶段 3 全部门控通过。阶段 4 应增加有滞回的 hunger/fatigue 高层意图和慢时标随机探索，
但继续保留 CNS readout、避障优先级和现有 motor command 路径。
