# Native → Three.js viewer：阶段 1

日期：2026-09-13。状态：实现完成，自动化 smoke 已通过；Windows 浏览器可视 FPS 待人工验收。

## 冻结边界

- CUDA MaleCNS、0.1 ms 神经步长、MuJoCo 0.2 ms 默认物理步长不变。
- `BrainBodyBridge`、sensory/motor mapping、行为逻辑、模型参数和世界资产不变。
- Windows Three.js 只显示 WebSocket 状态，不产生感觉输入。
- native 端保留原 MuJoCo 双眼相机、FlyGym retina 处理和 15 Hz summary 回传；主 WSLg
  窗口隐藏为 1×1，不再承担观察者显示。

## 实现

`flybrain-world web-view` 启动既有模拟 worker，并在 `127.0.0.1:8765` 发布：

- 一次性 `scene`：与 WASM 浏览器路径共用的 body/geom/mesh/camera 描述；
- 默认 30 Hz `frame`：sequence、epoch、545 个 body/camera pose 浮点数和只读 snapshot。

`web/native-view.html` 复用 `scene.js` 的 `FlySceneRenderer`。浏览器缓存相邻两帧，延迟一个
发布周期，对 body 位置作线性插值、四元数作 slerp，然后在独立 RAF 循环绘制。reset/epoch
切换会清空插值跨度，不跨重置混合姿态。

首轮可视检查发现 144 Hz 屏幕会令页面按 144 FPS 重复提交姿态，高速飞行时插值身体与未延迟
chase snapshot 还会产生时间点错位。修复后先以 30 FPS 验证重影消失；用户确认正常后将实际
绘制默认值设为 60 FPS（可用 `?fps=20..90`
覆盖），同步插值 chase 的 `root_position`/显示时间，并在主画面每帧显式清除颜色、深度和
stencil buffer。长期来回飞行/重复轨迹属于既有行为问题，本阶段不修改行为逻辑。

双眼面板复用同一次 native retina 捕获的 FlyGym 处理结果，只增加只读显示分支。为控制
带宽，预览取每隔一个像素的灰度值，左右眼拼接为 450×256、约 115 KB 的二进制帧，以原生
retina 的最高 15 Hz 更新；完整分辨率采样、ommatidia readings、summary 和进入
`BrainBodyBridge` 的时序均未改变。它不是浏览器 retina readback。

## 已完成验证

- Rust release CUDA 编译通过；`flybrain-world` 6 个测试通过、1 个 full-pack 测试按原设定忽略。
- Three.js 原测试和新增姿态插值/epoch reset 测试通过。
- 本机 full-CNS WebSocket smoke：RTX 5080、166,700 neurons、71 bodies、545 pose words；
  10 秒、291 个 snapshot 的 realtime factor 均值约 `1.056×`（范围 `0.958–1.134×`），
  native retina 后的 `visual_left` 为非零值，证明浏览器未替换 native sensory 来源。
- 本地 WebSocket 客户端剔除初始大 scene JSON 解析期后，连续 60 个间隔实测约 `29.18 Hz`，
  61 帧 sequence 均唯一且连续；
  浏览器显示性能仍以页面 RAF 计数为准，而不是网络消息间隔。

尚未在 Windows 可见 Chrome/Edge 会话中记录稳定显示 FPS，因此本阶段还不能宣称已经通过
20–30 FPS 人工门槛。也尚未开始浏览器 binocular retina 异步 readback 评估。

## 运行

WSL 终端 1：

```bash
target/release/flybrain-world web-view
```

WSL 终端 2：

```bash
npm --prefix web start
```

Windows Chrome/Edge 打开 `http://localhost:8080/native-view.html`。页面左上角分别显示连接、
RAF FPS、stream sequence、仿真时间和 realtime factor。若修改 WebSocket 地址，可使用
`?ws=ws://host:port`；监听地址用 `web-view --bind` 修改。
