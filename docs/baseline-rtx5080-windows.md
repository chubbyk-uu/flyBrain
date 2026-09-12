# 阶段 1：Windows RTX 5080 完整 CNS 基线

日期：2026-09-12。上游代码 `cdd3a127766ec184e19c4988fe12b6fd2cbc64fd`。
本阶段没有改动神经计算、模型参数、感官/运动映射或世界行为。

## 数据恢复与身份校验

执行 `python tools/restore_male_cns_pack.py`，通过当前 shell 代理下载作者线上 pack，
逐分块校验、重组后再次验证完整 NPY 哈希。保留发布 manifest 和原始分块，未修改数组。
随后执行 `python tools/audit_baseline_assets.py`。

| 数组 | SHA-256 |
|---|---|
| neuron_ids.npy | `ddfa789213c8c5066400c026b65c112c267781c2e3ab380a39dd437866e704d6` |
| row_ptr.npy | `b38878452b4871cdd8563f29463231dab67b61780a5ee540900811e3bbf41511` |
| destinations.npy | `3e61173a407dd01c23280782e13e9c619f2c33eac120585ab1ca4ad4a8286af9` |
| signed_counts.npy | `6b2b9918174273d67eaef951c2bfd72a25ae5a07d3c7861ab249c8ca7dd38421` |

四个数组哈希与现有 I/O artifact 一致。规模为 166,700 个神经元、24,469,412 条边、
120,260,398 个接触；其中 14,745,137 条兴奋边、9,724,275 条抑制边。
28 个 I/O 组的 2,447 个不同 ID 均存在。49 个身体/世界运行资源哈希全部通过。
所有资源及 WASM/JS/神经源码哈希见
[资源审计](../outputs/baselines/rtx5080-windows/assets-audit.json)。

## Windows 浏览器实测

在 Windows Chrome `152.0.7977.83` 的独立临时配置中，通过 WSL 静态服务器运行。
不是 Linux Chromium 或软件 GPU：WebGPU 返回 `nvidia / blackwell`、fallback=false，
Chrome 系统 GPU 信息确认为 NVIDIA GeForce RTX 5080，驱动 `32.0.16.1692`。
安全上下文和跨源隔离均为 true。Windows 上 powerPreference 被忽略的浏览器警告有保留；
实际设备识别独立于该提示。WebGL 场景渲染器为 ANGLE/D3D11，不能将它误写成 WebGPU 后端。

- `/neural-test.html` 七组检查全部通过：tiny fixture、零延迟、抑制、silencing、
  稀疏/跨窗口、活动队列、超过单维工作组限制的高扇出和完整 i16 范围。
- `/perf-test.html`：10,000 个窗口，500 Hz，20 秒仿真；Full CNS rendered，
  retinal rendering 开启，GPU timestamps 关闭，使用当前版本而非 preserved baseline。
- 完整 CNS 重置：运行至约 0.214 秒、181,810 次脉冲后重置，时间/计数均归零，
  `qpos/qvel/total_spikes` 与初始状态完全一致。此重置检查没有观察者渲染。

| 指标 | 结果 |
|---|---:|
| 仿真时间 | 20.000 s |
| 运行墙钟时间（不含加载） | 62.577 s |
| 实时倍率 | 0.319609× |
| 累计群体脉冲遥测 | 21,636,616 |
| 曾发放神经元遥测 | 15,476 |
| 每窗口 brain 平均时间 | 3.528 ms |
| 每窗口 GPU 提交/读回等待平均时间 | 3.293 ms |
| 每窗口总处理平均时间 | 6.212 ms |
| 最终飞行状态 | Cruise |
| 最终高度 | 185.174 mm |
| 100 ms 采样轨迹最大高度 | 189.422 mm |
| MuJoCo 警告计数 | 全部为 0 |

GPU timestamps 未启用，报告中的 gpu_ms=0 表示未测量，不代表 GPU 计算免费。
轨迹包含 Grounded、Takeoff、Cruise、Landing；记录到味觉与口器伸展共同出现的采样，
这里只记录基本输出，不将其换算成连续支持进食时长或生物行为验收。
最终 `JSON.stringify([qpos,qvel,total_spikes])` SHA-256 为
`2b5b23ff5f799b96e307899b0d58b4d0f4f5b323016b8b698a1859bcb325bc55`。
这是一次本机带渲染运行，视网膜按墙钟采样，不承诺不同机器的长期轨迹一致。

原始记录：[WebGPU QA](../outputs/baselines/rtx5080-windows/neural-qa.json)、
[完整性能及轨迹](../outputs/baselines/rtx5080-windows/full-cns-rendered.json)、
[截图](../outputs/baselines/rtx5080-windows/full-cns-rendered.png)、
[重置检查](../outputs/baselines/rtx5080-windows/reset.json)。

## 复现

启动 `npm --prefix web start`。在 Windows 用独立 `--user-data-dir` 启动 Chrome，
开启 `--remote-debugging-port=9222` 并打开 `http://localhost:8080/neural-test.html`。
Windows PowerShell 可执行仓库中的 `tools/windows_webgpu_baseline.ps1`：

```powershell
.\tools\windows_webgpu_baseline.ps1 -Mode qa -Output C:\path\to\new-qa.json
.\tools\windows_webgpu_baseline.ps1 -Mode perf -Windows 10000 -Output C:\path\to\new-perf.json
.\tools\windows_webgpu_baseline.ps1 -Mode perf -Scenario cns -NoVision -Timestamps -Windows 10000 -Output C:\path\to\new-profile.json
.\tools\windows_webgpu_baseline.ps1 -Mode perf -Scenario world -NoVision -Windows 10000 -Output C:\path\to\new-world-only.json
.\tools\windows_webgpu_baseline.ps1 -Mode reset -Output C:\path\to\new-reset.json
```

输出路径必须未存在。若从 WSL 发起，使用 `wslpath -w` 得到实际发行版的 UNC 路径。
自动化只调用现有页面/worker，不改生产 JS、WGSL 或 WASM。

## 性能解读与后续诊断

0.319609× 是包含 WASM/CPU 物理、神经计算、感官处理和显示的完整闭环速度，
不是 RTX 5080 神经内核的独立吞吐量。每推进 2 ms 仿真，平均墙钟约 6.258 ms。
脑体桥接约 3.528 ms，其中提交/等待/读回约 3.293 ms；其余部分按墙钟差值粗估约
2.73 ms。差值不是独立物理计时器的测量，不能全部归到 MuJoCo。

等待包括设备工作、提交、同步、浏览器调度和读回，现有记录没有将这些细分。
因此尚不能断言 GPU 内核慢、问题必然来自 WebGPU，或 CUDA 一定能达到实时。
MuJoCo 本身仍在 WASM/CPU 上以 10 kHz 步进；仅替换神经后端不会消除这部分开销。

### 追加性能拆分

同一 Chrome/RTX 5080 环境追加 10,000 窗口对照。所有完整 CNS 对照的累计脉冲、最终
状态哈希、位置和飞行模式完全相同，因此这里没有用改变模型状态换取速度。各次运行受
浏览器调度和机器负载影响，不用不同运行总墙钟做逐项相减。

| 对照 | 实时倍率 | total 平均 | engine 平均 | wait 平均 | GPU pass 平均 |
|---|---:|---:|---:|---:|---:|
| 完整 CNS，无观察者，无 timestamp | 0.259× | 7.643 ms | 3.811 ms | 3.701 ms | 未测 |
| 完整 CNS，无观察者，有 timestamp | 0.238× | 8.317 ms | 4.000 ms | 3.869 ms | 0.506 ms |
| world-only，无观察者 | 0.301× | 6.569 ms | 0 | 0 | 0 |
| 完整 CNS，渲染但关闭 retina | 0.226× | 8.741 ms | 4.064 ms | 3.925 ms | 未测 |

timestamp 对照的 GPU pass 中位数为 `0.328 ms`，p95 为 `1.180 ms`，最大值为
`4.981 ms`。平均 `0.506 ms` 明显小于平均 `3.869 ms` 的提交后等待，说明 RTX 5080
低利用率符合当前细粒度同步结构：每个 2 ms 控制窗口都提交一次并立即 map/readback，
浏览器调度、队列同步和读回占据了大部分 neural-engine 墙钟时间。GPU pass 时间不包含
全部主机准备、提交、映射和 MuJoCo 物理。

world-only 保持相同 10 kHz MuJoCo/WASM 物理，但其身体停留在地面，和完整 CNS 的飞行
轨迹不同，因此它证明 WASM 物理本身成本显著，却不能作为完整 CNS 的严格可减基线。
“渲染但关闭 retina”运行只记录到一个 presentation frame，说明 Chrome 将该窗口的
`requestAnimationFrame` 限流；该结果保留用于审计，但不能量化持续观察者渲染开销。
原先带 retina 的有效渲染运行记录了 3,884 帧，其 `0.320×` 仍是显示路径基线。

按 timestamp 运行做同次运行的上界分析：若把整个 `4.223 ms` brain 时间理想地消除，
其余 `4.094 ms/窗口` 对应约 `0.488×`；若只把 `4.000 ms` engine 时间替换成当前测得的
`0.506 ms` GPU pass，约为 `0.415×`。这只是浏览器结构下的 Amdahl 上界估算，不是
CUDA 性能承诺。CUDA 能显著缩短神经路径，但要突破浏览器物理上限，正式 CUDA 路线还
必须使用原生 Rust + CUDA + 原生 MuJoCo；原生闭环速度需接入后实测。

原始记录：[无观察者 CNS](../outputs/baselines/rtx5080-windows/full-cns-no-observer.json)、
[无观察者 GPU timestamps](../outputs/baselines/rtx5080-windows/full-cns-no-observer-timestamps.json)、
[world-only](../outputs/baselines/rtx5080-windows/world-only.json)、
[关闭 retina 的渲染对照](../outputs/baselines/rtx5080-windows/full-cns-rendered-no-vision.json)。

结论：阶段 1 及其性能诊断均完成。下一阶段是独立 CUDA 神经核心；核心阶段不接
MuJoCo/world，通过独立验收后再接原生 MuJoCo，采用 [项目规范](project-spec.md)
中的分层验收标准。
