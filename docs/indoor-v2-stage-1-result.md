# indoor-v2 阶段1：独立场景重建

状态：阶段1通过（支撑速度采用原项目20 ms位移估计口径，详见测量勘误）。日期：2026-09-13。

## 实现范围

`tools/build_indoor_v2.py` 从空 worldbody 新建环境，仅复制原果蝇子树、身体资产和
动力学/执行器/传感器契约，输出独立 `assets/neuromechfly/indoor-v2.xml`。
运行时不加载旧房间再隐藏家具。`--scene indoor-v2` 显式选择；默认切换留待运行控制完成。

- 室内有效范围300×220×140 mm，180×80×30 mm矮长茶几，无地垫、旧家具或室外装饰。
- 糖果中心 `[48,-12,30.6]`，花蜜 `[-48,12,65.4]`，盆栽放在茶几上。
- 默认身体出生位置 `[26,-12,32.1]`，近糖源但未接触；worker读取权威出生位置，不再搬糖。
- 新灰绿地板、米灰墙、浅木茶几、青色花盆、粉色花瓣、橙色糖果。Three.js程序纹理由本项目
  新写确定性算法生成；native retina使用同一几何及材质基色，但不声称浏览器纹理逐像素一致。
- 糖果和花瓣采用薄几何；花朵的碰撞支撑是半径7.5 mm、顶面65 mm的连续花托。花瓣、叶片、
  花蕊小球为显示几何，不逐片增加碰撞体。糖果位于平整桌面上，暂为非碰撞的薄食物表面。
- 糖源保留旧schema的 `movable` 兼容字段，但新版初始化不移动；该字段不表示自主导航权限。

## 已执行的检查

`python3 tools/test_indoor_v2.py`：3项通过。果蝇XML子树、actuator、sensor、contact、keyframe
结构完全一致；环境仅新材质，无旧detail/rug；桌腿0–26 mm与桌板26–30 mm连续；源与geom
中心一致，默认出生距离满足15–35 mm建议和味觉安全余量。

`cargo test --release --features cuda --lib indoor_v2_standalone_body_and_support_contract -- --nocapture`：
1项通过，内含地板、桌面、花托各5秒物理仿真；132 DOF、71 bodies、127 joints、56 actuators
保持，质量、惯量、关节类型和执行器连接逐项相同；无MuJoCo warning，无持续下沉，身体未穿过支撑面。

| 支撑面 | 末2秒20 ms位移估计速度中位数 (mm/s) | 同期瞬时速度中位数 (mm/s) |
|---|---:|---:|
| 地板 | 0.036093 | 2.129159 |
| 茶几 | 0.042007 | 2.173480 |
| 花托 | 0.036962 | 1.861531 |

测量勘误：初次新测试直接使用瞬时qvel，地板2.40 mm/s失败；修正地板接触对后仍约2.13。
现按原项目 `small_room_support_surfaces_settle_without_vertical_drift` 的20 ms位移估计口径，
判定长期支撑稳定性，门槛仍为2 mm/s；同时保留瞬时值，**不宣称瞬时抖动低于2 mm/s**。
两种统计不能混用。若后续需要瞬时速度也满足该阈值，应作为新增接触抖动要求处理。

`cargo test --release --features cuda --lib scene_layout -- --nocapture`：2项通过。
首次误用无cuda的lib测试及只测binary，均筛选出0项，不计为有效验收。
Python、JavaScript语法与 `git diff --check` 通过。

## 视觉证据

使用本机 Chromium + SwiftShader 打开真实 `native-gallery.html`，通过native WebSocket传入
MuJoCo编译后的几何，不是离线另画场景。仅场景检查用 `--no-brain`；不作为神经行为或Windows性能证据。

第一轮八张全景/四向/茶几/盆花/糖果截图已逐张亲自查看，位置与连接正确，无房外残留。
检查发现地板纹理摩尔纹、花叶过于平直，随后降低纹理对比并去掉规则噪点，倾斜叶片、增加花蕊细节。
第一轮截图位于 `outputs/indoor-v2/stage-1/`；第二轮 `review-2/` 仍有条纹。进一步确认条纹为
阴影自遮挡，缩小新版场景阴影覆盖范围并设置0.15 mm normal bias解决，阴影保持开启。
第三轮 `review-3/` 八张图片已逐张亲自复查：地面无条纹，家具/盆栽连接连续，房间内没有
无关装饰。最终截图及哈希随阶段提交；这些图片仅验收场景，不是行为成功证据。
图库限制空闲渲染为10 Hz且拍摄期间不重复提交，正式viewer渲染配置不变。

## 资源哈希

- indoor-v2.xml：`82255657080675694577055e39897c2e7fa3acd33ef666735ac946eb3210e6b0`
- indoor-v2.json：`706c5562396e3fc83dcc8381cd57c55a5e97909bad77950c041e97c7c06f2a73`
- indoor-v2-habitat.json：`18c836b20c6c7771de45f1a3dc1574e0c34786074ebc8c63fa0fbef46d8e7d87`

本阶段不包含此前未提交的起降/翼幅候选代码；不宣称20秒进食、300秒行为或正式Windows性能通过。
