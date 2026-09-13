# 阶段 6：室内美术、果蝇外观与神经翼显示结果

日期：2026-09-13

状态：**历史自动测试记录保留；最终视觉及长期行为验收未通过，需修复**。

2026-09-13后续勘误：实际网页截图确认桌面/支撑错位、房外叶片和装饰残留，图库相机仍有
旧位置；用户报告长时间盆底绕圈。旧用户对帧率/重影的确认不能替代新版场景视觉验收。
后续本机缓存Chromium已成功打开网页并截图，说明原“无法截图”的环境结论不能继续沿用；
软件渲染截图仍不证明Windows/RTX实际帧率。以下保留当时测试和限制描述，当前范围以
[修复计划](indoor-repair-plan-2026-09-13.md)为准。

## 实现与边界

- Three.js 继续直接显示 `small-room-v1` 中已有的茶几、糖/水果、花盆、叶片、花朵和室内
  细节；按房间、木材、陶瓷、土壤、叶片、花、果蝇几丁质、复眼和半透明翅分层材质。
  `PCFSoftShadowMap` 和 2048 阴影贴图保持开启，没有使用残影后处理。
- WebSocket scene 以向后兼容的可选 `bodies` 字段补充 body 名称、父节点和局部变换。旧 scene
  没有该字段时仍按既有绝对 pose 显示；新协议可把显示翅稳定挂在胸部，而不是显示 30 Hz
  采样到的 218 Hz 瞬时翼姿。
- native snapshot 新增只读 `wing_display`：包络来自实际 hybrid flight amplitude，物理频率
  来自 `218 Hz × flight_frequency_scale`，左右幅差来自实际 flight steering。浏览器使用连续、
  正向的 18 Hz 可见载波表达高速翼运动；它不是逐拍气动力重建，不写回 MuJoCo、retina 或 CNS。
- WebSocket 中断后翼包络回到中性收翼并每秒重连；native simulation 不依赖浏览器。页面暴露
  `window.flybrainAcceptance()`，用于读取实际 render count、p95 frame interval、连接与重连状态。
- `native-gallery.html` 自动抓取 room、table+sugar、plant+flower 固定视点，并且只在 native
  snapshot 真正出现 grounded、walking、flight、feeding、grooming 时收集对应卡片；左右 retina
  卡片来自 native 二进制预览，不伪造行为状态。
- `tools/run_native_viewer.sh` 同时管理 native simulation 和静态服务器，退出时清理两个进程。

## 性能修复与结果

定位发现隐藏 retina 原来按墙钟 20 Hz 提交。仿真出现短暂接触减速时，它会在每个生物秒内
过度采样，并持续占用 WSLg D3D12 context。现在提交上限以 MuJoCo simulation time 计时，且每次
采集后显式释放当前 GL context。双眼 450×512、三 PBO、左右交错、FlyGym ommatidia 处理、
summary 和 `BrainBodyBridge` 路径均未改变；reset 可清空新时钟。

RTX 5080 上的 300 秒 full MaleCNS + MuJoCo + native retina + WebSocket 结果：

| 指标 | 结果 |
| --- | ---: |
| 生物时间 / 墙钟 | 300.000 s / 297.047 s |
| 总 realtime | **1.010×** |
| CUDA brain wall / engine | 129.954 s / 103.719 s |
| MuJoCo physics / `mj_step` | 146.431 s / 142.036 s |
| native retina 更新 | 3702 |
| 客户端 pose | 8208，**28.44 Hz** |
| 客户端 retina preview | 3595 |
| 客户端 snapshot 局部 realtime 均值 | 1.032× |

机器可读结果为 `/tmp/flybrain-stage6-native-viewer-300s.json`，SHA-256
`69daa721a23aaeeee2e7ebefba44b2cac30d58a69e7942020b4fed47d277e6c7`。

Windows Chrome/Edge 无法从当前无人值守 Linux 进程启动：WSL interop 对 Windows EXE 返回
`Exec format error`，Playwright 也不为当前 Ubuntu 26.04 提供 Chromium。因而没有伪造 10 分钟
RAF 数值或截图。路线预先允许在这种情况下沿用用户已确认的 Windows 60 FPS、无重影和双眼
可见验收，并提供最终可打开的 `native-view.html` / `native-gallery.html`。144 Hz RAF 的纯自动
调度测试得到 60/90 FPS 目标，p95 间隔分别不超过 25/16 ms；真实显卡 RAF 仍可通过页面函数
复核。

## 冻结边界与行为回归

- 本阶段未修改 `assets/neuromechfly`、`world_sim.rs`、`BrainBodyBridge`、MaleCNS pack、行为参数、
  actuator 或 qpos。`fly.xml` 仍为
  `dc4b13e3cad341c8c06276d164e940503e4ed9954a0a2bf15720ea44a2835ecb`，habitat 为
  `b6525a321f3f971185f0e5481d3221e3b3d69fe8a0214a4190cf90bc338b92a3`，aerodynamics 为
  `834f5dbda6b41c1fa8baba87c02e9c03e77d68f886d8470e460c88ea20e39434`，gait 为
  `95e0042dd726ec620f161d1f9cb9f773292cd50596fb41392ab0ec950a4859a3`。
- 12 秒固定输入在阶段 5/6 的全部 sample 逐字段相同，规范化 sample SHA-256 均为
  `38dd645566c37481f87231279ac27fba387943a8152955626376b0e807e3d3be`。
- 阶段 5 统一门控再次读取为 PASS，SHA-256
  `d4566b1767b47048a14586af945bfc85282f445d8d3402afd37af193c39dea38`。三个 300 秒 seed
  11/13/17 仍分别为 `1.025×`、`1.022×`、`1.106×`，包含取食、落地休息、爬行、防长期同号
  转向和每次 8 个完整自主清洁动作。

## 验证

- `npm --prefix web test`：PASS，包括 mesh/retina、pose slerp/epoch、旧新 snapshot、翼门控及
  60/90 Hz frame limiter。
- Rust fmt 与目标 Clippy `-D warnings`：PASS；live viewer 4 项、native stream 1 项、grooming
  13 项、homeostasis 4 项测试全部通过。
- 全部 Rust lib 测试在修正 MaleCNS 新增组数断言后为 271 项通过；仅余 3 项因本地未提供历史
  `outputs/packs/flywire_v783` 而失败，与当前 MaleCNS 路径无关。
- appearance/flight/behavior trace/MaleCNS I/O Python 测试 21 项通过。
- `flybrain-world verify --scene small-room-v1 --steps 1000`：PASS，qpos/qvel hash 与既有场景
  基线一致。
- 新 native scene + frame WebSocket smoke 确认 body hierarchy 与 `wing_display` 均存在；
  `git diff --check` 通过。

历史结论（已被上述实际视觉审查限定，不再作为整体通过声明）：场景、完整身体、MaleCNS、native retina 与行为路径保持边界，显示层已完成美术分层、
平滑神经翼显示、稳定 60/90 Hz 限帧、自动重连和最终 gallery；阶段 6 可以提交。
