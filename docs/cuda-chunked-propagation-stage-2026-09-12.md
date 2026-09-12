# CUDA chunked CSR propagation 阶段报告

日期：2026-09-12。环境：WSL2/Linux、NVIDIA GeForce RTX 5080、CUDA Toolkit
13.0、原生 MuJoCo 3.9。本阶段只改变 CUDA 内部的 CSR 工作划分；MaleCNS 数据、f32、
CSR 边及权重、delay ring、refractory、tick 边界、sensory/motor mapping、行为逻辑、
模型参数和 MuJoCo 世界均未修改。

## 实现

MaleCNS 的 166,700 个 source neuron 中有 154,907 个具有出边，最大 out-degree 为
11,203。原 kernel 由一个 CUDA thread 串行遍历一个 source 的所有出边，因而高出度
source 会形成明显的线程内长尾。

新路径在引擎创建时按最多 256 条连续 CSR 边生成 propagation task。完整 MaleCNS 生成
183,690 个 task，增加约 2.20 MB 常驻显存；每个 task 保存原 source、CSR start 和 end。
kernel 仍先检查对应 delayed spike 和 silenced-source 标志，并用原有 i32 `atomicAdd`
写入 arrival。非零 delay 时，神经元 decay/threshold 与读取独立 delay-ring slot 的
chunked propagation 保持在同一个 kernel；`reset_store` 仍在其后执行。因此没有改变
神经状态更新顺序或 delay 语义。

`chunked-256` 在验收后成为默认路径。可用
`FLYBRAIN_CUDA_PROPAGATION=source-serial` 显式选择旧实现做回归或 A/B；非法值会直接
报错，不静默改变后端。

## 性能

同一 release 构建、0.2 biological seconds、500 Hz、无 settle 的完整 CNS + 原生
MuJoCo A/B：

- source-serial：CUDA engine `0.4012 s`，内部端到端 `0.6573 s`；
- chunked-256：CUDA engine `0.0885 s`，内部端到端 `0.3377 s`；
- neural engine 加速约 `4.53×`，内部端到端加速约 `1.95×`。

chunked-256 的 1 秒连续运行三次：CUDA engine
`0.4186/0.4166/0.4159 s`，中位数 `0.4166 s`，神经核心约 `2.40×` 实时；内部端到端
`1.2505/1.2468/1.2514 s`，中位数 `1.2505 s`，约 `0.800×` 实时。三次均为
population `1,022,346`、motor `4,059`，最终位姿完全一致。

与上一阶段 source-serial 的 1 秒中位数相比，CUDA engine 从 `4.0872 s` 降至
`0.4166 s`，约 `9.81×`；内部端到端从 `5.4458 s` 降至 `1.2505 s`，约 `4.36×`。
当前神经核心已经超过实时，但完整闭环仍差约 20%；下一步应重新 profile bridge、
MuJoCo、同步和 viewer，而不能继续假定 propagation 是唯一瓶颈。

## 正确性与门控

- 默认 chunked-256 与显式 source-serial 的四项 CUDA tiny/parity 测试均通过；首次
  f64 数值差异和“无 spike 分歧”结论未改变。
- 默认路径的完整 MaleCNS 预注册验收通过：自身重复/分块、15 个 population 与关键
  群体、20 ms spike/state 及已有实验门控全部保持原结果。报告为
  `outputs/cuda/male_cns_validation_chunked256_default_2026-09-12.json`，SHA-256：
  `a14f1febd52acdfce72d5327508f7eaf212e9682d80fb570ce6e60022c63b5ef`。
- 默认路径的 2 秒原生 world 成对门控通过：intact population `2,071,162`、motor
  `8,345`、flight `1.822 s`、forward `29.142 mm`；disconnected population
  `2,072,049`、motor `8,612`、保持 grounded。报告 SHA-256：
  `67c7a6c9c0a223163ef0de7e68e342b7eea15a2ac14331ec51d15b9520a27cfc`。
- 编译、严格 Clippy 和两种 propagation 模式的 CUDA tiny/parity 均通过。

## 下一阶段边界

下一阶段重新分项测量已优化默认路径的 bridge、MuJoCo、CUDA API 同步和 viewer。
优先处理完整闭环剩余约 20% 的差距；CUDA Graph 只有在新的 profile 证明 launch/sync
仍占主导时才进入实验。继续沿用同一固定输入和 world gate，不修改行为与世界。
