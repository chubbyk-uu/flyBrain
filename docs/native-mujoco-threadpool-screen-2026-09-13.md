# 现有身体的 MuJoCo 线程池与本机库筛选

日期：2026-09-13。状态：线程与本机库主筛选完成，未改变默认运行配置。

## 范围与方法

用户选择继续检查现有身体的优化空间，FlyGym 2.x 留作备选。本轮保持
`assets/neuromechfly` 的 132 DOF 身体、MaleCNS v1.0、感觉/运动映射、模型参数、
行为控制和世界；使用阶段 A 的物理 0.2 ms 候选，神经仍为 0.1 ms，控制为 500 Hz。
所有性能测试均为 headless，不含 GUI、双眼渲染，也不代表完整行为验收。

新增独立诊断库 `tools/mujoco_threadpool_probe.c`，通过显式 `LD_PRELOAD` 在首次
`mj_step` 前绑定线程池；0 workers 保持未绑定。每个 mjData 销毁后释放对应线程池，
支持程序启动时顺序创建/销毁模型，但不支持多个并发仿真实例或 viewer 数据复制。
不得与 `tools/mujoco_step_probe.c` 的非线程安全碰撞计数器同时加载。

`tools/benchmark_mujoco_threads.py` 使用同一 release 二进制、CUDA graph、完整 CNS，
每次 10 生物秒，默认 0.5 秒 settle、40 mm 食物距离；三轮正反顺序测试。
记录 wall time、物理/CNS 分项、首次非计时报告差异、同配置重复差异及资源哈希。
比较报告输出是诊断手段，不要求 CUDA 与 CPU-f64 长程逐神经元一致。

线程参数是额外 worker 数，不含调用线程。当前正式代码没有绑定 MuJoCo 线程池；
viewer 的模拟/显示线程解耦与单步内部线程池是两个不同机制。

## 官方库线程筛选

| 额外 workers | 总耗时中位数 | mj_step 中位数 | 实时倍率 |
|---|---:|---:|---:|
| 0 | 9.491 s | 5.466 s | 1.054× |
| 2 | 9.898 s | 5.825 s | 1.010× |
| 4 | 10.252 s | 6.059 s | 0.975× |
| 8 | 10.825 s | 6.531 s | 0.924× |

12 次测试全部非计时报告相同，每次 flight 4.868 s、FEED 0.662 s。
增加 2/4/8 workers 的 mj_step 耗时分别增加约 6.6%/10.8%/19.5%，因此不采用。
这组 10 秒样本中的 FEED 时长不足以证明已经通过至少 1 秒连续摄食的长程门控。

追加 0/1 worker 的三轮交替对照：0-worker 总耗时 9.466 s、mj_step 5.447 s，
1-worker 总耗时 9.990 s、mj_step 5.919 s；分别约 1.056× 和 1.001×。
六次非计时报告也全部相同。一个 worker 同样无收益。

原默认物理 0.1 ms 另做一对 10 秒补测：0-worker 为 16.266 s（0.615×），
2-worker 为 17.084 s（0.585×）；mj_step 分别 11.247/11.833 s，也无收益。
这对非计时报告相同，flight 5.156 s、FEED 2.980 s。仅一轮，不作为多轮统计；
未进行本轮的断连行为门控，也未把两种物理 dt 的不同输出当作数值故障。

本地 MuJoCo 3.9.0 源码 `src/engine/engine_forward.c::mj_fwdPosition` 表明，
线程池把惯量与碰撞分成两个任务；`mj_collision` 本身没有把所有几何对分配给多个
worker。约束侧可以按 island 并行。当前连接身体的任务结构并不适合按 CPU 核数
线性加速；观测到的减速与任务同步/调度开销相符，但尚未通过硬件计数器分解原因。

## 本机编译库

使用此前构建的同版本 MuJoCo 3.9.0 Release 库，编译参数包含
`-O3 -march=native -ffp-contract=off`，AVX/intrinsics 开启。
只通过子进程 `LD_LIBRARY_PATH` 选择库，`ldd` 确认解析到该文件；未改 conda、
默认软链接或 Rust 二进制。0-worker 三次总耗时 10.871/10.927/10.892 s，
中位数 mj_step 6.816 s、brain 3.888 s、实时倍率 0.918×。

相对官方库主筛选中位数，mj_step 耗时增加约 24.7%，端到端耗时增加约 14.8%。
三次非计时报告彼此相同，并与官方库报告相同，因此拒绝该本机库候选。
库间测试按组运行，没有交错库顺序；该结果不证明所有本机编译策略都会更慢。

## 后续方向

保持官方 MuJoCo 库且不绑定线程池。下一轮优先细分碰撞候选生成、BVH 遍历和
实际几何碰撞耗时，评估保持碰撞语义的保守筛选优化；此前关闭整个 midphase 已更慢，
不能重复当成未试过的优化。更换 DOF、身体和 FlyGym 当前不推进。
GUI 是独立的渲染瓶颈，本轮未测，不把 headless 接近实时当作 GUI 已流畅。

## 资源与复现

- 二进制 SHA-256：`da6f527456d4b6cb45f7122b5a7a394229c27c48047075a2a5b4d73c842caeb4`
- 官方 MuJoCo 库 SHA-256：`526773636a795dad11e094c8655d2375984a5cd7090f254d86bb71074651b852`
- 本机编译库 SHA-256：`e7d39f64a367286e0f1ec42c11a7cb8d03f4bd031345b29929234297f0072301`
- 线程筛选：`outputs/cuda/threadpool-screen-2026-09-13/stock-v2/summary.json`
- 单 worker 补测：`outputs/cuda/threadpool-screen-2026-09-13/stock-one-worker/summary.json`
- 本机库筛选：`outputs/cuda/threadpool-screen-2026-09-13/native/summary.json`
- 默认 dt 补测：`outputs/cuda/threadpool-screen-2026-09-13/default-dt/summary.json`
- 各报告 `initial_state.assets`、`pack_arrays` 和 `io_sha256` 保存身体、CNS 和 I/O 哈希。

```bash
cc -shared -fPIC -O2 -Wall -Wextra -Werror \
  -I work/mujoco-3.9.0-src/include tools/mujoco_threadpool_probe.c \
  -o /tmp/flybrain-threadpool-probe.so -L work/mujoco/lib -lmujoco -ldl
conda run -n flybrain python tools/benchmark_mujoco_threads.py \
  --probe /tmp/flybrain-threadpool-probe.so \
  --output-dir outputs/cuda/threadpool-new-run
```

输出目录必须不存在。早期 `smoke-2.json` 因诊断库编译失败而实际未加载，不作为
2-worker 证据；`stock/` 是诊断库未处理启动期 mjData 销毁重建的失败尝试，亦不计入。
已修复并通过正式 `stock-v2` 三轮；脚本检查加载标记，避免静默未加载被计作有效实验。

验证：诊断 C 库通过 `-Wall -Wextra -Werror` 编译；Python 工具 Ruff 检查与格式检查
通过；`git diff --check` 通过。共 23 次有效完整 CNS 运行正常退出；在同物理 dt 下
所比较的报告除计时外一致。未修改运行时 Rust 代码，因此本轮没有重跑 Rust 全套测试。
