# indoor-v2 阶段3：网页运行控制及状态显示

日期：2026-09-13。状态：生命周期与真实网页门控通过。

## 交付

- `web-view` 默认独立新场景 `indoor-v2`，仍是CUDA MaleCNS、神经0.1 ms/物理0.2 ms。
- 后台立即推进；仅正式viewer在场景及首帧可显示后发送 `viewer_ready`。后台worker持有一次性
  标记，串行处理并发ready；图库和普通诊断连接不发ready，不消耗自动重置资格。
- 首次ready完整reset并继续；暂停/继续幂等；手动reset完整恢复并暂停，不重新开放ready资格。
- reset重建CNS、重置身体、需求及控制器/缓存，恢复场景出生点、食物位置/启用状态、飞行开关；
  保留已配置的行为种子并从该种子重放，不使用上一次运行的随机进度。
- 网页按钮：暂停、继续、重置并暂停、全景、跟随。显示native饥饿、飞行疲劳、清洁冲动和
  生理起飞限制原因。清洁冲动目前兼容内部 `dirt`，节奏参数的更新在阶段6，不伪称已实现。
- retina二进制升级FBR2（24字节头，新增u64 epoch）；解码器仍能读FBR1，但正式viewer只显示
  匹配当前epoch的画面。服务器过滤旧epoch，网页重置时清空姿态插值、翼状态和retina。
  手动reset并暂停时双眼暂时空白，明确显示等待新epoch；继续后收到新的native画面。
- 暂停时物理、神经、需求值及显示振翅相位均冻结。生命周期通道不允许发送身体姿态、运动
  命令或修改感觉通路；WebSocket仍默认只监听127.0.0.1。

## 实测门控

`cargo test --release --features cuda --bin flybrain-world viewer_lifecycle -- --include-ignored --test-threads=1 --nocapture`
显式执行完整CNS与无脑两种配置，2/2通过：后台先运行5秒，重复ready只得到epoch1，暂停5墙钟秒
时间/神经累计计数/姿态/饥饿/疲劳/清洁冲动不变；手动reset得到epoch2，t=0、出生点正确、
hunger=0.72/fatigue=0.05/urge=0.10；重复ready仍暂停，resume恢复推进。

`node tools/test_viewer_lifecycle.mjs` 对完整CNS实际WebSocket通过。记录见
`outputs/indoor-v2/stage-3/websocket-lifecycle.json`：

- 普通诊断连接时t=5.000、epoch0，未提前触发reset。
- 两客户端同时ready：epoch1、t=0、运行中。
- 断连/新客户端/重复ready：仍epoch1；暂停5秒保持t=0.714。
- 手动reset：epoch2、t=0、暂停；ready不解除暂停；resume后t=0.114。
- 接收75条retina，包含epoch0/1/2，错epoch消息0条。

重启独立后台后，`node tools/test_viewer_browser.mjs` 用真实Chromium页面通过：首次打开自动
epoch1，实际点击暂停、刷新仍暂停/epoch1，实际点击reset得到暂停epoch2/t=0，新标签页不重置，
继续后时间前进。结果与截图位于 `outputs/indoor-v2/stage-3/browser/`；已亲自查看
`reset-paused-overview.png`，确认按钮/需求状态、桌面出生位置和新场景显示正确。

网页用Linux Chromium/SwiftShader做功能和视觉检查，截图约9 FPS不是Windows/RTX原生浏览器
性能结果，不可据此判阶段7性能通过或失败。已关闭本轮诊断网页的持续渲染，避免后续基准争抢。

JavaScript已有测试、新FBR1/FBR2解码/坏包检查、暂停翼相位测试通过。此前起降/翼幅候选改动
仍不纳入本阶段提交；相关速度/动作门控仍待阶段4–7。
