# 阶段 2：small-room-v1 物理、感觉与性能门控

状态：**通过**。测试主机为 WSL2、RTX 5080、MuJoCo 0.2 ms、MaleCNS 0.1 ms。

## 物理与感觉

- `ground_plane`、`table_top`、`flower_support` 分别从匹配表面高度开始运行 5 生物秒；
  最后 2 秒按 20 ms 采样的根部垂直速度中位数均不超过 `2 mm/s`。
- small-room habitat 定向测试确认糖与花蜜资源 ID、geom ID 不同；口器在各自味觉范围内只
  命中对应资源，移出范围后下一 20 ms 控制窗可清除味觉。
- 既有 `cns_feeding_requires_both_taste_context_and_mn9_spikes` 测试继续通过，伸吻仍要求
  物理 taste 与 MaleCNS MN9 同时存在。
- 两眼仍使用原生 FlyGym `450×512`、每眼 721 ommatidia 的相机和采样映射。实测 WebSocket
  snapshot 中左右 visual 均非零，8 秒范围为 `0.6053..0.9815`。

## 隐藏 retina 性能修复

WSLg D3D12 对隐藏 GLFW surface 的同步 readback 很慢。原实现还绘制无用主画面、串行读取
两眼，60 秒只能得到 237 次 binocular 更新（3.95 Hz）。现在隐藏 sensor：

- 不再画主画面；
- 使用三缓冲 OpenGL PBO，避免读取刚提交的 framebuffer；
- 左右眼以全分辨率交错采集，各自保留最新值，发布的每帧仍包含完整双眼图和双眼摘要；
- 隐藏感觉相机关闭 WSLg 上会强制串行的 shadow pass；Windows Three.js 可见场景的阴影
  没有关闭；相机投影、场景几何、ommatidia map 和感觉转导不变；
- 排除不可见 fluid/inertial 和已禁用 legacy 装饰的 render group。

## 性能与传输结果

- full-CNS headless 20 秒：15.541 s wall，`1.287× realtime`；CUDA engine 6.574 s、
  brain total 8.187 s、MuJoCo/flight physics 7.181 s，无 warning/NaN/reset。
- small-room native-retina + WebSocket 60 秒，以 `--speed 1.01` 验证余量：60.002 s sim /
  59.409 s wall = `1.010×`；内部 window 48.234 s；retina 804 次 = `13.53 Hz`。
- 同设置 legacy：内部 window 53.429 s；small-room 快 9.7%，不存在 10% 性能退化。
- 8 秒真实 WebSocket 客户端：pose 230 帧（28.69 Hz）、retina 111 帧（13.84 Hz），
  sequence 重复或倒退均为 0。
- Windows Three.js 默认 60 FPS、无重影沿用用户在阶段 0 已确认的显示基线；本轮没有伪造
  无人值守 RAF 测量。

## 验证说明

- `cargo clippy --features cuda --bin flybrain-world -- -D warnings` 通过。
- small-room 场景、味觉与三表面 5 秒定向测试通过。
- 全库测试仍只有三个缺少旧 `flywire_v783` pack 的既知环境失败；与本阶段无关。
