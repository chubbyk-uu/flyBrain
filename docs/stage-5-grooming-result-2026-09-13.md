# 阶段 5：自主搓腿—头眼清洁结果

日期：2026-09-13

状态：**通过全部预定义门控**

## 神经证据与实现边界

- 使用 hash 绑定的 MaleCNS v1.0 原始注释，SHA-256 为
  `2177e246113e4cfbf1e7772ec37c6da1955ff22e8063d0b1f833101f99a9a3b2`。
- 注释审计得到 64 个 `class=mechanosensory, subclass=grooming` 的 JO-FV/JO-FD1 神经元，
  以及左右各两个 aDN1/aDN2 下行神经元：左侧 DNg62 `13624`、DNge078 `14537`，右侧
  DNg62 `15148`、DNge078 `36541`。aDN1/aDN2 的触角清洁指令作用依据
  [Hampel et al., eLife 2015](https://doi.org/10.7554/eLife.08758)。
- dirt 是工程化 `[0,1]` 状态。它在饱食、真实地面条件下，以 5 Hz 上限驱动 JO grooming
  sensory，以 20 Hz 上限驱动 aDN command fixture；动作只有在读取到 aDN spike 后才能启动。
  这是可解释的混合门控，不声称恢复完整生物清洁回路。
- 动作只写现有前足 actuator：稳定四足支撑、停止行走、前足互搓、前足触达头/复眼、恢复
  站立；不设置 qpos 或身体位姿。饥饿、离地、接触危险或 motor/probe 断连会阻止或中断动作。

神经 I/O artifact SHA-256：
`b68e0f679084d4510f5b9d5fb773d9f68719e2948af57c126bdfd200adcb39e2`；证据 artifact
SHA-256：`5b349d0f573a7829f0b4f8846dbdb3bfb0e6e5ca5044788083e59971fa914f33`；参数
SHA-256：`aa8be0de8951b2b0304421417f4c24d8967720e7867dd48e70e072c97e5ae35f`。

## Fixture 门控

高 dirt、饱食、地面 fixture 在 0.586 秒启动，启动前已有 0.502 秒稳定支撑。连续清洁动作
持续 5.796 秒，其中包含三次 1.8 秒完整搓擦—头眼 sweep；每次完成使 dirt 分别下降
44.997%、44.996%、44.994%，之后恢复爬行并起飞。动作期间：

- 实际 MuJoCo 足部接触最少 4 足；
- 搓擦阶段两前足最小距离 0.194 mm；
- 随后的头眼阶段最小距离 0.146 mm；
- aDN probe 峰值大于 0，且前足 joint trajectory 明显非静止。

低 dirt/饥饿 30 秒 fixture、grooming probe 断连和全部 motor output 断连对照均未完成清洁；
probe 断连对照报告的 grooming rate 为 0。中断、少于四足、飞行中、暂停/reset、不同 dt、
滞回、序列化、固定输入确定性和 dirt 边界由单元测试覆盖；中断动作不会获得完整清洁降幅。

| Fixture | SHA-256 |
| --- | --- |
| 高 dirt、连通 | `0c1df2880025e42426d6d59303e6a0027035e739191ae122a1395e1bc440352d` |
| 低 dirt/饥饿 | `48fcc695c503ef42431d4d2fdef6f3c2ea933977f3b6193ce40f72646b4cd4d1` |
| grooming probe 断连 | `143f046686b80c2c978ce41e36dc74a7789deab56fcaeab61994c94bdea8535b` |
| motor output 断连 | `6bf2dcfc11c5b279ad1894d7d8d61986fa9450b488b73734f5594ae952398afe` |

## 三次 300 秒回归

| Seed | 完整清洁次数 | Realtime | 最长疑似跌倒 | 清洁中起飞 | 结果 SHA-256 |
| ---: | ---: | ---: | ---: | --- | --- |
| 11 | 8 | 1.025x | 0.020 s | 无 | `f8087cc741df42a3951b3f4a7086f0a9af160145f0d01381921b7b5dc3a08777` |
| 13 | 8 | 1.022x | 0.020 s | 无 | `653bd69d3d68f606a947c1f01927d9831d185ff812fe9c8aa25752cd10b234a6` |
| 17 | 8 | 1.106x | 0.020 s | 无 | `534190b839fd5f9d4facfb8d5435e31a07c46628cc6fe79dcf9ff7b431ed1d76` |

平均 realtime 为 **1.051x**，是阶段 4 平均 1.047x 的 100.4%，超过预定义的 95% 下限。
三次均有飞行、支撑休息、爬行、自主清洁，无 NaN、穿墙或永久跌倒。统一门控输出
`/tmp/flybrain-stage5-gates.json`，SHA-256
`d4566b1767b47048a14586af945bfc85282f445d8d3402afd37af193c39dea38`。

额外诊断：seed 11 的非重叠 20 秒圆周检测有三个相邻命中，但最长同号转向仅 4.32 秒，空域
覆盖率 20.15%，并持续发生取食、落地、爬行和清洁；这不是阶段 5 预定义门槛，原始诊断保留，
未通过事后修改阈值隐藏。

## 验证

- `cargo fmt -- --check`：PASS。
- grooming/homeostasis 单元与 world integration 目标测试：PASS。
- MaleCNS I/O builder 可从三份 hash 绑定 Feather 数据确定性重建新增组；所有 68 个新增 root ID
  均存在于冻结 pack，CSR 数组和神经元/边数未改变。
- `tools/evaluate_stage5.py`：fixture、三次长程及聚合门控全部 PASS。

结论：阶段 5 通过，可以进入阶段 6 显示美术、果蝇外观和翼运动。
