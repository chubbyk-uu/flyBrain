# 前腿前伸并拢互搓修正

2026-09-13。历史阶段报告：用户要求只修正互搓姿势；不改场景、需求节奏、MaleCNS、感觉通路、翻身或性能策略。本轮修正、相关测试及实际画面检查完成，后续已随 `683e41a` 提交；下文哈希及测量只属于当时版本。

## 修改及门控

旧目标仅在眼睛前0.25 mm，靠关节2/6反向摆动会横向分离，而且容易藏在头下。改为真实身体逆运动学拟合的两个端点：眼睛前0.75/0.95 mm，左右各离中线0.025 mm；在同一窄线上以5 Hz前后交替滑动，而不是横向甩开。目标值只是工程控制命令，须以真实MuJoCo身体姿态验收。

先沿已验证的折脚路径抬脚卸力，再逐渐前伸；直接从支撑面向前伸的首个候选在约0.43秒被支撑保护中止，不算通过。保留4秒整套动作、其中约1.5秒完整前伸互搓，再接原擦头/眼及落脚。擦头关键姿态和8 Hz小幅刷动不变。

预定义的新姿态门槛（互搓完整前伸段）：双脚端位于头部前方至少0.45 mm；横向间距全程小于0.15 mm；两脚端三维距离小于0.35 mm；实际前后相对行程大于0.15 mm，持续记录至少1.4秒。四条中后足实际支撑，完成后恢复运动。地板和桌面均检查；完整CNS的正向/断开清洁输入/断开运动输出对照保持配对，正面、侧面、俯视必须亲自查看。

## 验证记录

运行文件SHA256：`eeb4b25b72109039b6dcb7960b641b8bc461e1e1717013b81e90503a1ef79c19`。场景XML仍为`575df5a7830d46ad7bd6dcfb19a5fa0b8c03046cb810d53b67222502ee2488e2`。

- [20项相关Rust测试](../outputs/indoor-v2/forward-rub/checks/flybrain-forward-rub-v2-tests.log)通过，地板/桌面手动触发物理fixture中前伸最小0.968/0.969 mm，横向间距最大0.099/0.110 mm，脚端三维距离最大0.211/0.215 mm；不把手动fixture当作自主CNS触发证据。
- [完整CNS成对门控](../outputs/indoor-v2/forward-rub/causal-gate.json)通过：地板/桌面均自主完成4秒动作、四条中后足实际支撑；断开清洁输入或运动输出时均无清洁事件，同组初始状态和运行文件哈希一致。
- [地板真实姿态](../outputs/indoor-v2/forward-rub/floor-geometry.json)和[桌面真实姿态](../outputs/indoor-v2/forward-rub/table-geometry.json)均通过新几何门控。前伸最小0.962/0.975 mm，横向间距最大均约0.116 mm；两脚端三维距离最大0.214/0.211 mm，实际前后相对行程0.350/0.348 mm；50 Hz记录覆盖1.50/1.48秒完整前伸互搓段。横向间距与三维距离不同，后者还包含前后交替滑动，不能混淆。
- 已亲自查看真实native姿态回放的[俯视互搓](../outputs/indoor-v2/forward-rub/visual-table/top/080.png)、[侧面](../outputs/indoor-v2/forward-rub/visual-table/side/080.png)、[另一滑动相位](../outputs/indoor-v2/forward-rub/visual-table/side/083.png)、[正面并拢](../outputs/indoor-v2/forward-rub/visual-table/front/080.png)和[后续擦头](../outputs/indoor-v2/forward-rub/visual-table/front/155.png)。脚端已伸出头部轮廓，并拢成窄尖端，随后收回擦头；不使用浏览器假肢或改动身体mesh。
- [侧面原时间视频](../outputs/indoor-v2/forward-rub/visual-table/side-native-time.mp4)、[正面原时间视频](../outputs/indoor-v2/forward-rub/visual-table/front-native-time.mp4)按50 Hz真实姿态生成，非性能测量。保持原4秒总时长及神经/需求触发规则。

复查命令（输出路径须不存在）：

```bash
cargo test --release --features cuda --lib grooming
node tools/check_forward_rub.mjs outputs/indoor-v2/forward-rub/connected/table-11.json /tmp/forward-rub-recheck.json
```

只运行本次相关的动作和因果检查，不声称重新执行了全部寻食或300秒长程验收。

性能备注：用户说明上一轮低实时率时正在运行其他负载；这次不重新做性能调查，不把受负载影响的历史0.54×测量归因于灯光，也不宣称测得了新的无负载实时率。
