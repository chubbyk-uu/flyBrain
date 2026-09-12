# 阶段 4：饥饿、疲劳与探索混合控制结果

日期：2026-09-13
状态：**通过全部预定义门控**

## 实施边界

- 保持 MaleCNS v1.0 的 166,700 个神经元、24,469,412 条边、f32/CSR/delay ring/
  refractory/执行顺序，以及 0.1 ms 神经步长不变。
- 保持 MuJoCo 0.2 ms、132 DOF/127 joints/56 actuators、BrainBodyBridge、既有
  sensory/motor mapping 和原生 retina 路径不变。
- 新增版本化 `[0,1]` hunger/fatigue 状态及确定性探索航点；工程状态只调制既有感知、
  着陆、起飞抑制和 CNS steering，不直接设置身体位姿，也不从资源真实坐标伪造视觉识别。
- hunger 仅在 taste 接触、稳定支撑、实际伸吻、MN9 活动和 motor output 连通同时成立时下降。
  fatigue 仅在有功飞行时增加，在稳定支撑时恢复；疲劳着陆沿用既有 landing 路径。

参数文件 SHA-256：
`4a15427156871ec4cbbff6c8020b734d0e326fc3a55348cd6e6a71082db8dc28`。
小室内场景 SHA-256：
`9d757e53808849e059670efea083d0525228e92fc3f987f2cf81c350cdf8ae05`。

## Fixture 门控

使用 release CUDA world，固定行为种子，并由 `tools/evaluate_stage4.py` 判定：

| Fixture | 结果 | 关键证据 | 原始结果 SHA-256 |
| --- | --- | --- | --- |
| 近距糖，8 s | PASS | 取食 1.810 s；hunger 0.720 降至 0.300；MN9 和伸吻均实际出现；离开糖源 | `cdfef2f65d2f5e13cdd4532d8ea4ed9980cfc627b61931edeb103ffc7a9d5f35` |
| 花蜜，28 s | PASS | ORN 驱动 APPROACH/DESCEND；接触花蕊、伸吻、取食 1.890 s 后离开 | `ffa852ebf7c5d67f2d71466dfde2d74e42a6be10b0c23722c94806833c04f218` |
| motor disconnect，5 s | PASS | 无取食、hunger 不下降、无主动探索 steering | `38c91fe31d0f1d11546f5f92fd7c688d1e40f0539da6d63d3fee1d436d64fd54` |

纯糖 fixture 只把无气味糖块放到口器前方，不使用远距气味。花蜜 fixture 使用 MaleCNS ORN
响应进入既有 odor-guidance 路径；接近高度门槛和近距 dwell 避免刚起飞即错误着陆。

## 三次 300 秒回归

| Seed | Realtime | 取食 | 最长飞行 | 支撑休息 | 支撑爬行 | 空域覆盖 | 最长同号转向 | 连续 20 s 圆周窗口 | 最大 mode 占比 |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 11 | 1.013x | 3.036 s | 45.852 s | 14.470 s | 26.936 s | 18.33% | 6.800 s | 1 | 76.54% |
| 13 | 1.019x | 3.020 s | 45.162 s | 14.320 s | 27.336 s | 21.36% | 7.500 s | 2 | 76.46% |
| 17 | 1.108x | 1.970 s | 42.740 s | 14.240 s | 26.806 s | 20.45% | 5.496 s | 1 | 76.14% |

三次均有飞行、疲劳着陆、至少 3 秒连续支撑休息、爬行和成功取食；无 NaN、physics
warning、穿墙或自动 reset。平均 realtime 为 **1.047x**，高于 0.95x 门槛。阶段 3 覆盖率
基线为 11.21%；seed 11 虽低于绝对 20%，但提高到 18.33%，符合预先声明的“相对阶段 3
提高或达到 20%”门槛，其余两个 seed 均超过 20%。圆周门控按相邻、互不重叠的 20 秒块计算，
避免一次持续事件被重叠滑窗重复计数。

原始结果及 SHA-256：

- seed 11：`/tmp/flybrain-stage4-final-seed11-300s.json`，
  `0ae793b4e089bd3e81afae06fb3a7904a9994a8808576af20fd81632e3bafa8e`
- seed 13：`/tmp/flybrain-stage4-final-seed13-300s.json`，
  `804d9ef206dd3793a2e6895ad89dfa312a188306f8b984528859f1ba389850a9`
- seed 17：`/tmp/flybrain-stage4-final-seed17-300s.json`，
  `bf6b3282d539fd67389a24aff921f6ccb4905737e255d6aea0679f44b420fc38`
- 汇总门控：`/tmp/flybrain-stage4-gates.json`，
  `38fd110dfd2c962a97459ac5c2929254862c96d853e46b188ce26234594200c0`

## 验证

- `cargo fmt -- --check`：PASS。
- `cargo clippy --features cuda --lib --bin flybrain-world -- -D warnings`：PASS。
- `cargo build --release --features cuda --bin flybrain-world`：PASS；binary SHA-256
  `2f90478ecd7bb33502be94759aa482b29c09887200736e2fed34ad93a9afe802`。
- `target/release/flybrain-world verify --scene small-room-v1 --steps 1000`：PASS。
- homeostasis、odor guidance、baseline artifact 与行为分析目标测试：PASS。
- `cargo test --features cuda --lib` 的 269 个测试中，266 个通过；仅 3 个既有测试因本地缺少
  `outputs/packs/flywire_v783` 历史 pack 而失败。失败均不涉及 MaleCNS v1.0、阶段 4 代码或
  当前版本化资产，未用跳过测试来隐藏该缺口。

结论：阶段 4 通过，可以进入阶段 5 自主搓腿—头眼清洁。
