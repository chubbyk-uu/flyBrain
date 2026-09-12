# CUDA probe 缓存与瓶颈复测阶段报告

日期：2026-09-12。环境：WSL2/Linux、NVIDIA GeForce RTX 5080、CUDA Toolkit
13.0、原生 MuJoCo 3.9。本阶段继续只优化 CUDA/host 交界；MaleCNS 数据、f32、CSR、
delay ring、refractory、tick 执行顺序、sensory/motor mapping、行为逻辑、模型参数和
MuJoCo 世界均未修改。

## 实现

- `CudaEngine` 缓存固定 probe 索引和上一个窗口的累计 spike count。probe 集合不变时，
  后续窗口不再重复上传索引，也不再读回 before，只读回 after 并在 Rust 侧继续使用
  checked subtraction。
- 稠密执行会使该缓存失效；probe 集合改变时重新上传并从设备建立新的 before 基线。
- population total、active neuron count 和 voltage sum 仍使用同一个确定性归约内核，
  但三个标量合并为一个对齐结构，只执行一次 device-to-host copy。

## Profile 结果

Nsight Systems 使用相同的 50 ms、500 Hz、完整 CNS + 原生 MuJoCo 协议。优化前后
`cudaMemcpy` 调用数由 `154` 降至 `92`，减少 `40.3%`；kernel launch 由 `1,466`
降至 `1,442`，少掉的是窗口间不再需要的 before-probe gather。profile 下
`brain_engine_seconds` 由 `0.1738 s` 降至 `0.1121 s`。WSL profile 未提供 GPU kernel
timeline，因此这些数据只能证明 host API/同步路径减少，不能单独给出各 kernel 的设备
执行时间。

未开启 profiler 的 0.2 秒短测受共享 GPU 时钟和其他 Windows GPU 进程影响，三次
`brain_engine_seconds` 为 `0.6264/0.6102/0.4821 s`，不作为稳定加速比。更长的 1 秒
固定运行三次结果稳定：内部端到端 `5.4458/5.4918/5.4140 s`，中位数 `5.4458 s`
（约 `0.184×` 实时）；CUDA engine 中位数 `4.0872 s`。三次均为 population
`1,022,346`、motor `4,059`，最终位姿完全一致。

这说明低活动的短窗口会低估持续闭环成本。传输次数已经明显下降，但随 CNS 活动增长，
当前“一条 CUDA thread 处理一个 source neuron，再串行遍历该 source 的全部出边”的
CSR propagation 成为下一主要候选瓶颈。Nsight Compute 抽样因主机未开放 NVIDIA
performance counters（`ERR_NVGPUCTRPERM`）而无法取得硬件计数器，不能据此声称已经
测得 occupancy 或 memory-bandwidth 数值。

## 正确性与回归

- 四项 CUDA tiny/parity 测试通过；首次 f64 数值差异仍为 tick 2、neuron 1 的
  conductance，最大误差和 spike 无分歧结论均未改变。
- 完整 MaleCNS 预注册验收通过：自身重复/分块、15 个 population 与关键群体、短程
  spike/state 和已有实验门控均保持原结果。报告 SHA-256：
  `486e436be867f09d90da9c7dfa8a0ebd3da8ff12d405745b9e22db68f5525f09`。
- 2 秒原生 world 成对门控通过：intact population `2,071,162`、motor `8,345`、
  flight `1.822 s`、forward `29.142 mm`；disconnected population `2,072,049`、
  motor `8,612`、保持 grounded。报告 SHA-256：
  `a8194800702622b6cf58ddfa228d81d9b1969ffa5db74fc81a56098ddc7429dd`。
- `cargo check`、四项 CUDA 测试和严格 Clippy 均通过。

## 下一阶段边界

下一阶段先为 propagation 增加不依赖特权硬件计数器的 CUDA event 分项计时，并设计
active-source/edge-parallel 的最小实验实现。该实现必须保留 i32 arrival 累加、每 tick
读写边界和现有执行顺序，并先在独立 benchmark 与 tiny fixture 上证明自身确定性和
spike parity；在此之前不替换默认完整 CNS 路径，也不开始新世界或清洁行为。
