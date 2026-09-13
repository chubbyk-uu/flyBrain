# 阶段6：需求闭环与自主合并清洁

日期：2026-09-13。**本阶段门控通过；阶段7的五条300秒及Windows性能门控尚未完成。**
判据沿用[实施前登记](indoor-v2-stage-6-protocol.md)，不是只看动作计数。

## 改动边界

CUDA MaleCNS、神经0.1 ms、物理0.2 ms、完整身体、pack、BrainBodyBridge神经绑定、
native retina均不变。修改工程需求、清洁准备/动作控制、重置初值和诊断记录。
饥饿增长改为`(0.55-0.30)/60`；仅真实支持面进食降低饥饿，空中靠近不能算。
有功飞行/悬停均累积飞行疲劳，爬行和静止均恢复；取消必定静止3.5秒。
疲劳恢复期可按种子选择2–4秒的爬行/静止机会，而不是强制每次相同休息动作。

清洁冲动兼容内部`dirt`字段，独立种子产生初次10–15秒、完成后10–20秒机会；
完成后冲动保留10%，冷却5秒计入间隔。只保留一次待执行机会，不补做积攒的次数。
合并搓前足、擦头/眼为2.5秒动作；安全、饱食、神经门控共同决定实际启动。
地板与茶几按实际支撑判断，不再用绝对高度5 mm排除茶几。

必须说明：既有桥接按冲动刺激清洁感觉组和aDN命令组，再读取脉冲门控；需求变量及
动作模板是工程实现。“自主”不是宣称连接组自行恢复了真实清洁动机。

## 定量结果

| 门控 | 实测 |
|---|---|
| 无进食0.30→0.55 | 60秒±一个2 ms窗口 |
| 初始疲劳0.05、功率0.85，速度60/0/0.5 mm/s | 均32.728秒达到0.68 |
| 初始疲劳0.72，静止/4 mm/s支撑爬行 | 均10.000秒恢复起飞资格 |
| 5种子各20个清洁机会 | 首次10.068–14.718秒；后续10.002–19.980秒；逐种子精确重放 |
| 实际CNS地板自主动作 | 0.530秒开始，2.500秒完成；中后四足支持 |
| 实际CNS茶几自主动作 | 0.532秒开始，2.500秒完成；中后四足支持 |
| 实际CNS搓足/头眼最短距离 | 两面约0.177/0.502 mm，分别在预登记阶段测量 |
| 清洁门控断连、运动输出断连 | 两面均无自主启动、无完成事件 |

完整配对结果及每行初态/运行文件哈希见
[机器门控](../outputs/indoor-v2/stage-6/grooming-gate-v4.json)。同一运行文件SHA-256：
`3b8afdd744bfd5cac3a1fb658dc64b50a85fcdfe64c17397ad4593e35ba5d0cf`。
配对初态哈希归一化被干预的连接开关；实际开关另在`brain.grooming_probe_connected`和
`brain.motor_outputs_connected`记录，不能把归一化哈希解释为干预输入也完全相同。
所有短程组及90秒试运行均没有“非实际进食却降低饥饿”的窗口。
手动受控身体测试另覆盖走动后准备、实际四条中后足、完成后恢复爬行；不计为自主证据。

检查共18项grooming、6项homeostasis、2项baseline及3项起降回归通过，
包括五次地板/五次茶几起降、55 mm着陆和近食物着陆。
[测试日志](../outputs/indoor-v2/stage-6/checks/)。
阶段5四类任务的种子11回归均通过，首次有效进食仍为10.492、7.496、11.260、5.282秒，
恢复失败窗口均0；这是四条回归，不冒充本阶段重新跑了全部20条。
[回归结果](../outputs/indoor-v2/stage-6/foraging-regression-v4/results.json)。

## 真实画面审查与修正

亲自查看了实际CNS姿态回放的准备、搓足、擦眼、收回阶段，地板和茶几均核对。
此前小偏移模板即使距离<1.5 mm，画面仍像前足停在地上，因此没有采用为最终动作。
修正采用原身体的离线运动学拟合；擦眼目标选实际眼球网格外侧而不是体坐标原点。
保留身体、关节弹簧和执行器，实际物理再验证，不用渲染器补画假动作。

- [茶几搓足侧面](../outputs/indoor-v2/stage-6/visual-v4/table/side/040.png)、
  [擦眼侧面](../outputs/indoor-v2/stage-6/visual-v4/table/side/090.png)、
  [搓足俯视](../outputs/indoor-v2/stage-6/visual-v4/table/top/040.png)、
  [擦眼俯视](../outputs/indoor-v2/stage-6/visual-v4/table/top/090.png)。
- [地板准备](../outputs/indoor-v2/stage-6/visual-v4/floor/side/010.png)、
  [搓足](../outputs/indoor-v2/stage-6/visual-v4/floor/side/040.png)、
  [擦眼](../outputs/indoor-v2/stage-6/visual-v4/floor/side/090.png)、
  [收回](../outputs/indoor-v2/stage-6/visual-v4/floor/side/133.png)。
- [茶几慢放](../outputs/indoor-v2/stage-6/visual-v4/table/slow-review.mp4)、
  [地板慢放](../outputs/indoor-v2/stage-6/visual-v4/floor/slow-review.mp4)：
  50 Hz记录真实姿态，25 FPS回放，左右为俯视/侧视。原始姿态来自自主事件，未手动触发。

额外修正：左右关节轴已经镜像，指令不能再次反号；活动期间冻结参考站姿，防止叠加
偏移每2 ms累积；准备时以0.2黏附让上一迈步姿态自然调整，避免完全钉住或无黏附跳动；
中足先前移、前足黏附渐退再抬起。此前姿态丢失支撑、启动迟到的失败保留在本地
`stage-6/cns-grooming-v2`和`/tmp/flybrain-phase6-grooming-physical-v*.log`，未覆盖原始结果。
截图是软件渲染视觉证据，**不是Windows RTX 5080性能证据**。

## 90秒自然运行：范围有限的闭环检查

默认出生/需求、种子11，无手动行为：5.282秒验证开始吃糖，7.088秒吃饱；
7.364秒起飞、41.496秒疲劳请求降落、44.856秒实际落地；45.350–47.850秒自主清洁，
随后出现实际休息，55.896秒解除疲劳起飞限制，56.052秒再次起飞；67.126秒重新饥饿。
首个清洁机会10.240秒产生，但飞行期间推迟，未积攒多次动作。
实际恢复资格比连续支撑fixture稍慢，源于真实接触并非每窗连续稳定；没有改仿真时钟。
这条只吃到糖，不能代替阶段7要求每条300秒都吃到两种食物。

[原始90秒记录](../outputs/indoor-v2/stage-6/needs-pilot-v1/seed-11.json.gz)。
完整CNS短测的原始JSON以gzip保留于`cns-grooming-v4`、`grooming-probe-off-v4`、
`grooming-motor-off-v4`，各含地板/茶几文件；连接组包含原始身体姿态及场景。

## 复现

```bash
cargo build --release --features cuda --bin flybrain-world
cargo test --release --features cuda --lib grooming -- --nocapture
cargo test --release --features cuda --lib homeostasis -- --nocapture
node tools/run_indoor_grooming.mjs outputs/recheck-grooming connected
node tools/run_indoor_grooming.mjs outputs/recheck-grooming-probe probe-off
node tools/run_indoor_grooming.mjs outputs/recheck-grooming-motor motor-off
node tools/summarize_grooming_gate.mjs outputs/recheck-grooming outputs/recheck-grooming-probe outputs/recheck-grooming-motor outputs/recheck-grooming-gate.json
```

目标输出必须不存在。`tools/fit_grooming_keyframes.py`使用已有Python环境的mujoco/numpy，
只输出拟合命令，不修改模型；没有安装依赖、改变conda环境。
阶段7继续检查双源转移、重复/停滞、每种子扰动对照、异常视频及前台浏览器10分钟性能。
