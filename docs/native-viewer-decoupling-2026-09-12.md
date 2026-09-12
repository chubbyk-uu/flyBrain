# 原生 viewer 解耦

日期：2026-09-12。本阶段实现 native viewer 的模拟/显示解耦，尚未解决 WSLg 的全部绘制开销。

## 线程边界

- `view_worker.rs` 在后台线程创建、推进和销毁 `SimulationStepper` 及 GPU 后端。
  神经/物理步长、控制窗口、MaleCNS、映射、模型和行为控制代码保持原样。
- 窗口及 OpenGL 留在主线程。后台按显示目标频率发布最新的完整 MuJoCo 数据快照；
  主线程复制至独立显示数据后释放短锁，所有绘制和像素回读都在锁外进行。
- 帧槽只有最新状态；命令队列上限 64 条，视觉输入只有最新一项，神经显示历史上限 1000 点。
  显示停顿不会积累旧帧队列。未添加预测姿态或插值，显示的是已计算的状态。
- 暂停、重置、糖源、飞行和搓脚命令在控制窗口之间应用。重置递增 epoch，丢弃旧 epoch
  的视觉输入并清空显示历史。正常退出及窗口错误都通知后台退出并 join；启动/运行错误会回传。
- 显示目标默认 60 FPS，标题分别报告实际 FPS 和仿真倍率。眼图采样最高 15 Hz 墙钟时间，
  其摘要在下一个可用控制窗口应用；隐藏眼图不停止脑连接情况下的视觉输入。

视觉采样调度已变化，交互运行不保证与旧 viewer 的神经/行为轨迹完全一致。
固定输入的 headless 路径没有改动。GPU 设备不跨线程移动，没有新增 GPU 后端的 unsafe Send。

## 验证

- 严格 Clippy：`cargo clippy --lib --bin flybrain-world --features cuda -- -D warnings` 通过。
- `cargo test --bin flybrain-world --features cuda`：6 个测试通过；另一个 GPU 测试默认忽略。
- 显式运行 `cargo test --bin flybrain-world --features cuda worker_preserves_full_cns -- --ignored --nocapture`：通过。
- 无显示器的 10 个控制窗口（20 ms）对照中，后台/直接推进的 time、qpos、qvel、ctrl、
  sensordata 完全一致；完整 MaleCNS/CUDA 同条件下这些状态及最终窗口 population/MN9
  spike 数、累计活跃神经元数和滤波群体发放率也一致。这不是长程逐神经元验收。
- 自动验证暂停后时间不推进、暂停中重置、旧视觉输入丢弃、恢复推进、无渲染消费者时运行、
  worker 退出和缺失资产的启动失败。实际 GUI 的所有按键未逐个自动注入测试。
- release 完整 CNS/双眼/神经面板 GUI 在 5 秒模拟后自动退出，无 GPU/线程错误。

## 实际显示瓶颈

窗口上下文通过 `glGetString(GL_RENDERER)` 确认为 `D3D12 (NVIDIA GeForce RTX 5080)`，
并非软件光栅化。短测显示：

| 条件 | 实际显示 | 同次运行末段仿真倍率 |
| --- | ---: | ---: |
| 原阴影，5 秒模拟 | 3.1 FPS | 0.80× |
| 关闭阴影，3 秒模拟 | 13.2 FPS | 0.77× |

保留阴影的另一轮分项测得常见主画面约 142–148 ms、双眼约 171–190 ms、graph/swap
约 2.6–3.1 ms；关闭阴影后，主画面约 36–60 ms，采样帧双眼约 46–71 ms，复用眼图的
非采样帧双眼面板约 0.2 ms。计时为 CPU 墙钟，包含驱动等待，不是 GPU timestamp。
这些是短测末段统计，未做同初态的旧/新 viewer 长程 FPS 配对，也不应把仿真倍率差异当作优化收益。

因此，解耦已经解除模拟阻塞绘制的依赖，但 MuJoCo OpenGL/WSLg 绘制本身仍很慢。
此前“解耦即可像网页一样流畅”的判断不充分；还需要针对原生渲染管线优化。

## 运行

```bash
target/release/flybrain-world view
FLYBRAIN_VIEWER_SHADOWS=0 target/release/flybrain-world view
FLYBRAIN_PROFILE_VIEWER=1 target/release/flybrain-world view --max-seconds 5
```

关闭阴影为可选诊断/预览条件，默认保持开启。它影响主画面和双眼像素，必须标记为不同视觉
输入条件；身体几何、质量、惯量、碰撞及神经参数均未改变。后续先剖析/优化原生绘制，
再回到 MuJoCo 实际仿真吞吐；线程解耦本身没有实现 1× 实时或保证 60 FPS。
