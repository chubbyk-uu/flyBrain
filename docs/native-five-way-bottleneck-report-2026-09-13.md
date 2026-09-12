# 原生 CNS / MuJoCo / viewer 五组性能定位

日期：2026-09-13。暂停功能开发，仅增加诊断开关、计时和独立采样工具。
没有改变身体、MaleCNS、神经动力学、感觉/运动绑定、物理步长、模型参数或行为控制算法。

## 实验边界

- WSL2、i7-13700F、RTX 5080，NVIDIA 驱动 616.92；原生 MuJoCo 3.9.0 官方库。
- 原始 132 DOF 身体；神经和物理均为 **0.1 ms**，500 Hz / 2 ms 控制窗口。
- 每组 10 生物秒；headless 恰好 5000 窗，viewer 浮点时间终止产生 5001 窗（10.002 s）。
- 默认 0.5 s settle、40 mm 食物距离；默认 1280×800、chase 相机、60 FPS 目标、speed=1。
- 保留阴影、4096 阴影贴图设置和原始双眼 450×512 RGB 捕获，未使用降质参数。
- OpenGL renderer 实测为 `D3D12 (NVIDIA GeForce RTX 5080)`，不是软件光栅化。
- 五组主测试显式设置 `FLYBRAIN_CUDA_EXECUTION=graph`，chunked-256 propagation。
  未设置环境变量的程序仍默认 direct；本报告不把 graph 称作全局默认。
- 主测试各两轮，正序/反序运行，无并发基准；另外各一轮 CUDA event/API 诊断。
  下表两轮采用算术平均，样本数不足以报告可靠的置信区间。

① headless 没有双眼渲染，仍使用既有 ray/velocity 等感觉代理；② `--no-brain`
保留双眼显示，但身体轨迹和接触负载不等于有脑条件；③ 正常完整 CNS viewer；
④ 完整 CNS viewer 禁用双眼捕获、回读、处理、显示及视觉 mailbox 更新；
⑤ 完整 CNS viewer 关闭 population/field telemetry 和对应脑图，保留运动 probe 与 retina。
④ 明确改变了实际视觉输入，属于用户要求的性能消融，不是行为等价优化。

加载 CNS/创建窗口的启动时间不作为吞吐指标。worker 计时从初始化完成后开始；显示
FPS 从 viewer 创建完成后统计，两者起点不同。显示线程与模拟线程并行，不能相加。
用户提供的 M3 Max / 浏览器速度作为历史背景，本轮没有重新测量，也不是严格硬件 A/B。

## 五组主测试

下列 brain、engine、physics、mj_step 均为每 10 生物秒的累计 CPU wall 秒。
engine 是 CUDA 后端调用墙钟，包含提交、传输和等待；不是 GPU kernel 活跃时间。

| 条件 | 模拟墙钟 s | 实时倍率 | 显示 FPS | brain 总计 s | CUDA engine s | physics s | 其中 mj_step s |
|---|---:|---:|---:|---:|---:|---:|---:|
| ① full CNS headless | 16.225 | 0.616× | — | 4.659 | 3.833 | 11.468 | 11.273 |
| ② no-brain viewer | 12.434 | 0.804× | 4.88 | 0 | 0 | 12.330 | 12.260 |
| ③ full CNS viewer | 16.437 | 0.609× | 3.90 | 4.929 | 4.020 | 11.380 | 11.188 |
| ④ full CNS / no retina | 16.419 | 0.609× | 8.97 | 4.950 | 4.041 | 11.339 | 11.149 |
| ⑤ full CNS / no telemetry | 16.141 | 0.620× | 4.05 | 4.660 | 4.018 | 11.356 | 11.166 |

headless 两轮 CUDA engine 为 3.432/4.234 s，存在运行间变化；physics 则为
11.536/11.399 s。不能将小幅脑耗时差异全部归因于 retina 或 telemetry。
no-brain 仍有约 12.3 s 物理成本，但它是不同接触轨迹，不能从两组相减估算 CNS 开销。

额外 direct 启动方式补测（各一轮，不混入上述平均）：headless 16.222 s、0.616×，
engine 3.776 s；full viewer 17.187 s、0.582×、3.85 FPS，engine 4.264 s。
直接启动与 graph 都表现为相同的显示瓶颈；这组样本不足以重新判定 graph/direct
总体收益。原始记录在 `direct-check/summary.json`。

## 显示帧分项

单位为平均每个显示帧的 **CPU wall 毫秒**，不是 GPU timestamp。

| 条件 | main 更新/绘制/HUD | 双眼渲染提交 | 双眼 readback | 双眼 CPU 处理 | 双眼总计 | graph/swap | 整帧 |
|---|---:|---:|---:|---:|---:|---:|---:|
| ② no-brain viewer | 91.54 | 0.71 | 107.62 | 1.01 | 110.83 | 1.91 | 204.29 |
| ③ full CNS viewer | 113.35 | 0.69 | 135.94 | 1.00 | 139.42 | 2.72 | 255.49 |
| ④ no retina | 107.76 | 0 | 0 | 0 | ~0 | 3.05 | 110.81 |
| ⑤ no telemetry | 109.95 | 0.71 | 130.96 | 1.01 | 134.50 | 2.11 | 246.57 |

双眼总计还包括 inset 上传/blit/overlay；main 含场景更新、主渲染、HUD 和图表数据准备。
两眼每次回读共 450×512×3×2 = 1,382,400 bytes。`mjr_readPixels` 的墙钟等待
包含此前排队的 GPU 工作、渲染完成和驱动同步，**不能把 136 ms 解释为搬运这 1.38 MB
的纯带宽耗时**。同理，0.69 ms 的渲染提交不表示双眼 GPU 渲染只花这么久。
未取得 OpenGL GPU query 或 Windows 图形驱动时间线，尚不能严格拆分纯 GPU 绘制与等待。

完整 viewer 的 126 个主测试帧全部捕获两眼；虽然采样上限是 15 Hz，但当前显示远慢于
此上限，所以几乎每帧都会回读。禁用 retina 实测捕获数为零，FPS 约提升 2.30 倍，
仍受约 108 ms 的 main 路径限制，远未达到 30/60 FPS。
本轮默认配置复现约 4 FPS，而非用户此前约 7 FPS；不把不同显示配置/统计区间当成回归。

## CUDA 神经、传输与同步

Nsight Systems 2025.3.2 的 smoke trace 取得 CUDA API 记录，但没有 GPU kernel/memcpy
活动表。诊断日志明确提示当前 CUDA driver 13.4 不受该构建支持，回退使用 13.1 tracing
库；这是已观测的兼容性线索，不断言它是缺失 GPU 活动的唯一原因。未更改驱动或安装工具。

使用 `tools/cuda_window_probe.cpp` 的独立 LD_PRELOAD 诊断库补测：
围绕每次 graph launch 记录 CUDA events，在原有 stream synchronize 完成后读取时间，
不另加 device synchronize；分别记录 cudaMemcpy 的方向、字节数和 CPU wall 时间。
GPU graph span 包含图内执行及调度间隙，不是所有 kernel 活跃时长的总和。

下表来自单独的一轮诊断运行，各项为每约 10 生物秒的累计秒：

| 条件 | GPU neural graph span | H2D API wall | D2H API wall | stream sync wall |
|---|---:|---:|---:|---:|
| ① headless | 2.716 | 0.411 | 0.348 | 2.742 |
| ② no-brain viewer | 0 | 0 | 0 | 0 |
| ③ full CNS viewer | 2.919 | 0.668 | 0.490 | 3.012 |
| ④ no retina | 2.934 | 0.676 | 0.488 | 3.024 |
| ⑤ no telemetry | 2.921 | 0.678 | 0.242 | 3.016 |

GPU graph 时间与 stream sync 等待高度重叠，**不能相加**。H2D/D2H 是同步 API 调用
耗时，包含驱动和等待，不是 DMA engine 的纯传输时长。CUDA event 诊断存在观测开销，
不使用这轮结果替代前面两轮普通计时；不能跨表把每列相加当作端到端。

### 每个 2 ms 窗口到底传了什么

正常 viewer 诊断：5001 graph launches、5001 有效 GPU event 测量、5002 stream sync
（额外一次为销毁时），**0 次 cudaDeviceSynchronize**。

- H2D：15,000 次，共 1,751,140 bytes，单次最大 404 bytes。稳定窗口上传 sparse
  lanes、counts、21 个 u32 offsets；初次 probe 索引被缓存。
- D2H：6,202 次，共 5,030,800 bytes，单次最大 **1,000 bytes**。
  常规 probe 为 250 个 u32 spike counts；另外是小型 population/field metrics。
- 关闭 telemetry 后 D2H 变为 5,002 次、5,002,000 bytes，恰好少 1,200 次和
  28,800 bytes，即 100 Hz field + 20 Hz population 的 24-byte metrics。
  回读的运动 probe 保留，完整 CNS 仍执行 5001 个窗口。
- 计数从首次 graph launch 开始，不包含 pack 初始化上传、首次 probe 索引及首窗 H2D。

**没有每窗全量状态拷贝。** 全量状态接口仍存在，含 spikes、累计 counts、voltage、
conductance，对 166,700 神经元共 2,167,100 bytes；实际 world 路径调用的是
`run_sparse_window` → `flybrain_cuda_run_sparse_probed`，非该全量接口。

graph 模式每窗调用 `cudaStreamSynchronize(engine->graph_stream)`，随后同步拷回
probe。源码中的 cudaDeviceSynchronize 属于其他 dense/sparse 接口或零 probe 分支。
默认 direct 的正常非零 probe 路径依赖同步 cudaMemcpy 等待结果，不应因为缺少显式
device synchronize 就声称不存在同步。旧版全状态传输问题已经在此前阶段修复。

## 瓶颈排序及下一步定位建议

1. **画面：main 绘制路径 + 同步双眼回读。** 无脑仍不足 5 FPS；关闭 retina 到约
   9 FPS，但 main 自身仍约 108 ms。下一步可做主画面/双眼阴影与 MSAA 的独立消融、
   GPU timestamp/驱动等待定位、异步 readback 实验；本轮未实现这些优化。
2. **模拟：MuJoCo physics。** 正常 full viewer 约 69% 模拟墙钟位于 physics。
   按本次相同物理负载，即使把神经耗时降为零，11.38 s / 10 生物秒的物理成本仍不满足
   1×。因此不能只优化 CUDA 解决原 0.1 ms 配置的实时倍率。
3. **CUDA：小块高频同步调用有延迟成本，但不存在大带宽状态回传。** 正常 viewer
   神经图约 0.584 ms/窗口；传输调用需要进一步研究合并/异步策略，但不得直接删除
   保证运动读出正确的依赖。神经与物理串行闭环依赖也限制无延迟并行。
4. **Telemetry 优先级较低。** 主测试 brain 总计减少约 0.269 s/10 生物秒，
   端到端约快 1.8%，FPS 仍约 4；不能靠关闭脑图获得流畅显示。

## 复现与验证

运行工具：`tools/benchmark_world_components.py`。诊断开关仅用环境变量：
`FLYBRAIN_BENCH_NO_RETINA`、`FLYBRAIN_BENCH_NO_TELEMETRY`（设置即启用）。
它们用于无人交互基准；正常交互仍保留 V/B 键原操作。

```bash
conda run -n flybrain python tools/benchmark_world_components.py \
  --output-dir outputs/cuda/five-way-new-baseline
g++ -shared -fPIC -O2 -Wall -Wextra -Werror -I /usr/local/cuda/include \
  tools/cuda_window_probe.cpp -o /tmp/flybrain-cuda-window-probe.so \
  -L /usr/local/cuda/lib64 -lcudart -ldl
conda run -n flybrain python tools/benchmark_world_components.py \
  --output-dir outputs/cuda/five-way-new-events --trials 1 \
  --cuda-probe /tmp/flybrain-cuda-window-probe.so
```

输出目录必须不存在。CUDA event probe 仅用于 graph、单个 CUDA 仿真线程，不能当成
通用 CUDA profiler。它在进程退出时交由驱动清理少量 event 资源。

- 主计时：`outputs/cuda/five-way-profile-2026-09-13/baseline/summary.json`
- CUDA 分项：`outputs/cuda/five-way-profile-2026-09-13/cuda-events/summary.json`
- 两轮主计时 summary SHA-256：`141e82aba633a127b27393184bb0d6b0027ababb649d39aa8f7555f22c574348`
- CUDA 分项 summary SHA-256：`cab5d54249b1cfa5eb9079252144368f056577b9333d78c4cfe6bc88cc737f24`
- Nsight 诊断：同目录 `nsys-smoke.nsys-rep` / `nsys-smoke.sqlite`
- 二进制 SHA-256：`59bfb6900dc245347ea7aea7c015f10417f6b2e1d6c195a5f7f3ef28fee16741`
- CUDA probe SHA-256：`eefd9788f42fa038cc7f6da4081174685f131ea9e45fd58aaf4f211d02086b26`
- 原始资产、pack、I/O 哈希保存在 headless 报告 `initial_state` 中。

普通 headless 两轮和 CUDA event probe 的 headless 报告除计时外完全相同。
retina 消融未捕获任何双眼图像；telemetry 消融少了预期 1200 次 metrics 回读。
严格 Clippy、bin 常规 6 项测试，以及显式启用的完整 CNS worker/direct 固定输入对照
均通过。Ruff 与诊断库严格编译通过。没有声称不同实时 retina 采样运行的轨迹必须一致，
也没有在本轮重做长期行为门控。

## 0.2 ms viewer 后续定位

用户接受 0.2 ms 为原生实时物理默认后，使用完整身体、完整 CNS 和正常 retina 做单轮
5 秒显示 A/B。4096/4× 基线约 2.89 FPS；1024/2× 约 2.91 FPS；即使降至 256/0×仍约
2.81 FPS。关闭阴影的诊断上界约 10.85 FPS，同时关闭 retina 约 17.44 FPS；无脑且两者
关闭约 25.04 FPS。640×400 与 1280×800 结果接近，关闭主场景高细节组也无收益。

细分计时显示 scene update 约 0.03 ms/帧、render 调用提交不足 1 ms、retina CPU 处理约
1 ms。长等待最初记在 HUD 和 `mjr_readPixels`；关闭 HUD 后同一等待转移到
`glfwSwapBuffers`，说明它是 OpenGL→D3D12 队列同步而非 HUD 字符绘制。`glxgears` 同环境
约 187 FPS，故不是 WSLg 的固定刷新上限，而是 MuJoCo classic OpenGL 的阴影 pass、三视角
和同步回读组合触发的慢路径。降低 shadow map 尺寸并不能解决；正式默认仍保留阴影，
全关只作为诊断。下一步需要异步 retina readback，并评估复用现有 Three.js 异步渲染器、
由原生 CUDA+MuJoCo 只发布状态的显示架构；后者尚未实施。
