# 自主混合控制果蝇：连续实施与验收路线

日期：2026-09-13。状态：**历史六阶段记录；新场景与整体行为验收重新打开**。
原“六个阶段全部完成”结论不足以覆盖用户实际场景和长期行为要求；网页截图已确认模型脱节，
用户报告300秒绕盆，运行中状态与起飞禁令问题吻合。当前执行依据为
[全新室内场景与行为修复计划](indoor-repair-plan-2026-09-13.md)。
下文保留原门槛及当时的“通过”记录，不代表通过新门槛，不删改历史实验数据。
原执行约定为：
每阶段只有全部硬门槛通过后才提交并进入下一阶段。历史性能实验和已放弃的身体简化路线
仍保留在其他报告中，但不再决定本轮顺序。

## 1. 最终目标与范围

在较小的室内场景中运行完整 CUDA MaleCNS + 原生 MuJoCo 果蝇，使它能够在神经活动参与的
混合控制下寻找并舔食糖或花蜜、自由飞行、落地休息、地面爬行、避免长期循环转圈，并在
合适状态下执行一套合并的搓腿—清理头部/复眼动作。饥饿、疲劳、积灰和探索意图允许使用
明确标注的工程状态；工程状态选择目标或调制神经驱动，不能直接传送身体或用预录动画替代
CNS 运动输出。

本轮顺序固定为：

1. 新室内场景骨架；
2. 场景物理、感觉与性能验收；
3. 300 秒既有行为诊断；
4. 饥饿、疲劳与探索混合控制；
5. 自主搓腿—头眼清洁；
6. 场景、果蝇外观与翼运动美术完善。

浏览器 binocular retina 异步 readback 不属于本轮任务。Windows Three.js 始终只显示；
神经感觉继续来自 native MuJoCo 双眼相机和 FlyGym retina。

## 2. 全程冻结项与通用门槛

下列硬门槛适用于每个阶段：

- MaleCNS pack 保持 166,700 个神经元、24,469,412 条边、f32 状态、CSR、delay ring、
  refractory 和既有 tick 执行顺序；每阶段记录并对照 pack manifest/hash。
- 神经步长保持 0.1 ms；MuJoCo 物理步长保持默认 0.2 ms；控制窗口保持 2 ms/500 Hz。
- 既有 `BrainBodyBridge` 感觉/运动通道语义不得静默改变。新增需求或清洁通道必须追加为
  有来源、有测试的版本化映射，不能重编号或删除现有绑定。
- 完整 NeuroMechFly 身体、132 DOF、127 joints、56 actuators 和已有 native retina 分辨率、
  15 Hz 采样上限保持；本轮不恢复已失败的减 DOF/primitive collision 候选。
- 每次自动验收无 NaN/Inf、无自动 reset、无新增 MuJoCo warning；CNS motor disconnect
  对照不得保留同等主动运动。
- `cargo fmt --check`、目标 Rust Clippy `-D warnings`、相关 Rust/JS/Python 测试和
  `git diff --check` 必须通过。全仓既有、与阶段无关的失败单独披露，不冒充新回归。
- 每阶段产生版本化配置、机器可读结果和简短报告；生成型大文件默认留在 `outputs/`，报告
  记录其路径与哈希。阶段提交不包含临时日志、缓存、凭据或代理设置，不自动 push。

性能统一在 RTX 5080/WSL2、无已知外部 GPU 负载下测量。至少一次 20 生物秒 warm run；
最终行为阶段使用固定种子长程运行。`realtime_factor` 和浏览器 FPS 分开报告。

## 3. 阶段 1：新室内场景骨架

状态：**已通过**。证据见
[`stage-1-small-room-result-2026-09-13.md`](stage-1-small-room-result-2026-09-13.md)。

### 实施

- 建立版本化 `small-room-v1` 场景配置，不覆盖 legacy 场景；CLI/manifest 明确记录选择。
- 房间边界缩小，场景只保留必要的地面、四壁和天花板；所有长度使用 MuJoCo 当前毫米约定。
- 放置一张茶几、一个糖果资源、一盆植物、至少一朵具有花蜜接触区的花。
- 地面、茶几顶面和花蕊附近提供明确的可支撑/降落碰撞面；装饰花瓣和叶片不必全部碰撞。
- 新家具优先使用静态 geom，不增加动态 DOF；视觉 geom 与碰撞 geom 使用可审计命名。
- 定义 `sugar`、`nectar` 两个不同资源 ID、接触区域、气味类型和显示材质。纯糖不默认具有
  强远距气味；花蜜可由花香提供远距工程感觉线索。
- 保持初始果蝇位置、身体、神经和行为代码不变；只为防止出生穿模允许调整场景出生点。

### 硬验收门槛

- legacy 与 `small-room-v1` 均可由显式 CLI 选择并加载；新场景配置有 schema/version/hash。
- 新场景包含且唯一解析：room bounds、coffee table、sugar、plant、flower、nectar zone。
- 果蝇 body/joint/DOF/actuator/sensor 数量与 legacy 完全相同；新增动态 DOF 为 0。
- 所有静态物体 AABB 位于房间边界内；初始身体与静态碰撞体无穿透。
- 糖和花蜜的资源 ID、geom/site 名称不同；可见表面与实际接触区中心误差不超过 1 mm。
- 自动 scene contract 测试、MuJoCo load/forward 以及 1 秒无脑 smoke 全部通过。
- Three.js scene descriptor 能显示所有必需对象，body pose 协议长度与场景声明一致。

通过后提交：`Add small indoor room scene skeleton`。

## 4. 阶段 2：场景物理、感觉与性能验收

状态：**已通过**。证据见
[`stage-2-small-room-gates-2026-09-13.md`](stage-2-small-room-gates-2026-09-13.md)。

### 实施

- 为地面、茶几和花蜜平台建立落体/支撑探针；检查接触数、穿透、速度和稳定时间。
- 分别把口器置于糖、花蜜接触区内外，验证 taste resource 的进入和离开边沿。
- 从固定出生点和固定视角记录 native 左右眼预览、强度/对比度以及对象可见性。
- 跑无脑物理、完整 CNS headless、完整 CNS + native retina + WebSocket 三组性能。
- 验证 WebSocket scene、pose、snapshot、双眼只读预览协议和浏览器页面。

### 硬验收门槛

- 地面、茶几、花蜜平台各连续稳定支撑至少 5 生物秒；末 2 秒质心垂直速度绝对值
  中位数不超过 2 mm/s，无持续穿透增长或接触爆炸。
- 口器进入 sugar/nectar 区后 20 ms 内出现对应 taste ID，离开后 20 ms 内关闭；另一资源
  不得误触发。固定接触的味觉→CNS→MN9→伸吻既有门控继续通过。
- 固定视点下左右眼均为非零图像，左右眼不得交换；糖、花和茶几在指定可见性 fixture 中
  至少覆盖一个有效 ommatidium。这里只证明可见，不宣称 CNS 已识别物体。
- 20 秒 full-CNS headless 平均 `realtime_factor >= 1.00`；60 秒 native retina + WebSocket
  平均 `>= 1.00`，且相对阶段 0 同模式退化不超过 10%。
- WebSocket 新鲜 pose/snapshot 稳态 25–30 Hz，无重复或倒退 sequence；双眼预览 10–15 Hz。
- Windows viewer 沿用用户已确认无重影的路径，默认 60 FPS；自动协议/页面 smoke 必须通过，
  若无人值守环境不能读取 Windows RAF，则明确标记为沿用已确认基线，不虚构新测量。
- 20 秒完整运行无非有限值、无 physics warning、无服务队列持续增长。

通过后提交：`Validate small room physics sensory and performance gates`。

## 5. 阶段 3：300 秒既有行为诊断

状态：**已通过**。证据见
[`stage-3-behavior-diagnosis-2026-09-13.md`](stage-3-behavior-diagnosis-2026-09-13.md)。

### 实施

- 在 `small-room-v1`、现有行为逻辑完全不变的条件下运行至少 300 生物秒。
- 固定记录位置、姿态、接触、flight/behavior/foraging mode、左右转向、landing drive、
  walking/flight readout、气味、味觉、视觉摘要和资源距离。
- 自动分割飞行、支撑、爬行、取食和重复轨迹 bout；检测圆周、折返、边界吸附和状态抖动。
- 对最长飞行和最明显重复轨迹回溯首次触发源与持续门控，给出代码位置和遥测证据。

### 硬验收门槛

- 单次连续记录不少于 300.0 生物秒，采样间隔不大于 20 ms，丢失/倒退时间戳为 0。
- 报告每类 mode 的驻留时间、bout 数、最长时长、转向符号连续时长、路径长度、净位移、
  房间占用栅格、接触/取食次数及最长无支撑飞行。
- 重复轨迹检测至少包含：10 秒窗口曲率/转向偏置、空间栅格循环、往返自相关三种指标；
  阈值在读取结果前写入工具并由合成 fixture 单测。
- 对“来回飞/循环飞行”给出至少一条由状态条件→仲裁→motor command 的可复核因果链；若数据
  否定原假设，也必须列出被排除的候选，不能只描述视频观感。
- 本阶段生产代码的行为输出哈希在同固定输入短程 fixture 中与阶段 2 一致；只允许增加
  记录/分析工具，不允许调行为参数。

本阶段的“通过”表示诊断证据完整，不表示问题已经修好。通过后提交：
`Diagnose long-run flight and exploration loops`。

## 6. 阶段 4：饥饿、疲劳与探索混合控制

状态：**已通过**。证据见
[`stage-4-homeostasis-result-2026-09-13.md`](stage-4-homeostasis-result-2026-09-13.md)。

### 实施

- 新增归一化 `[0,1]` 的 hunger/satiety 与 fatigue 状态，速率和阈值进入版本化参数。
- 只有 taste 接触、伸吻达到门槛且 CNS MN9 活动存在时才减少 hunger；进入 FEED 状态本身
  不能自动吃饱。离开食物和活动随时间恢复 hunger。
- fatigue 仅在实际飞行功率/运动存在时增加；稳定支撑休息时恢复。高 fatigue 调制 landing
  意图并抑制再次起飞，但空中不能瞬时断升力或直接设置位置。
- 探索使用固定种子、有限相关时间的目标方向/航点；避障和安全优先。圆周/折返检测只改变
  高层探索意图，不逐帧覆盖 CNS steering，也不读取资源真实坐标作为“视觉识别”。
- 饥饿时选择糖或花香感觉目标；饱食时允许爬行、休息或自由探索。使用滞回和最短驻留时间
  防止 mode 每个控制窗抖动。

### 硬验收门槛

- 状态单测覆盖边界、不同 dt、暂停/reset、固定种子、滞回和序列化；所有状态始终在 `[0,1]`。
- 固定 taste fixture 中，缺少接触、伸吻或 MN9 任一条件时 hunger 不下降；三者齐全时单调
  下降。CNS motor disconnect 后不得完成同等取食或主动探索。
- fatigue fixture 中连续飞行单调增加、稳定支撑单调恢复；高疲劳个体在 30 生物秒内进入
  landing，形成支撑后连续休息至少 3 秒；任何飞行 bout 不超过 90 秒。
- sugar 定向 fixture 和 flower/nectar 定向 fixture 各至少完成一次接近、接触、神经参与的
  伸吻和离开；纯糖 fixture 不依赖未声明的远距气味。
- 三个固定种子各运行 300 生物秒：每次至少包含一次飞行、一次有支撑休息和一次爬行 bout；
  至少两次运行成功取食；无单一 flight/walk/turn mode 占据超过总时长 85%。
- 任何同号转向不得连续超过 12 秒；20 秒滑窗的闭合圆周重复不得连续出现 3 个窗口；
  占用栅格覆盖率相对阶段 3 提高或至少达到可用地面/空域的 20%。
- 三次长程均无 NaN、physics warning、穿墙或自动 reset；平均 realtime 不低于 0.95×，
  若低于 1.00×须定位新增 CPU 成本并证明不超过总墙钟 5%。

通过后提交：`Add hunger fatigue and non-circular exploration control`。

## 7. 阶段 5：自主搓腿—头眼清洁

状态：**已通过**。完整证据见
[`stage-5-grooming-result-2026-09-13.md`](stage-5-grooming-result-2026-09-13.md)。

### 实施

- 新增单一 `[0,1]` dirt 状态；随时间、行走/飞行和环境接触累积，只在实际清洁动作期间下降。
- 审计 MaleCNS v1.0 注释和可用运动输出，建立版本化 grooming probe/mapping 证据。工程状态
  可以提供清洁意图，但动作启动必须同时满足可解释的 CNS 活动门控。
- 清洁作为一个可中断流程：稳定支撑→降低行走→前足相互搓擦→前足触达头部/复眼→恢复站立。
- 使用现有前足关节和真实 MuJoCo 接触/距离，不直接覆写身体位姿。饥饿、失去支撑、碰撞危险
  或起飞优先级可中断清洁。

### 硬验收门槛

- dirt 更新、reset、暂停、阈值/滞回、打断和固定种子测试通过；dirt 永远位于 `[0,1]`。
- 低 dirt、饥饿、飞行中或少于四足稳定支撑时，30 秒 fixture 中不得启动自主清洁。
- 高 dirt、饱食、地面稳定 fixture 中 30 秒内启动；开始前至少稳定 0.5 秒，动作持续
  1–8 秒且能够回到可行走姿态。
- 清洁过程中至少四足保持支撑；两前足先出现相互距离小于 1.5 mm 的搓擦阶段，随后至少
  一只前足与头/复眼目标距离小于 1.5 mm；这些阈值由身体尺度 fixture 验证。
- 完成一次未中断动作后 dirt 至少下降 30%；未实际达到搓擦和头眼阶段不得下降同等数值。
- grooming CNS probe 断开或全 motor outputs 断开时，不得完成同等动作；报告完整 CNS 与
  断连对照的 probe rate、关节轨迹和接触差异。
- 三个固定种子各 300 秒至少两次运行出现自主清洁，且无清洁期间起飞、身体爆炸、穿透、
  永久跌倒；平均 realtime 不低于阶段 4 的 95%。

通过后提交：`Add CNS-gated leg and head grooming behavior`。

## 8. 阶段 6：美术、果蝇外观与翼运动

状态：**已通过**。完整证据与无人值守 Windows 验证边界见
[`stage-6-visual-polish-result-2026-09-13.md`](stage-6-visual-polish-result-2026-09-13.md)。

### 实施

- 在不改变碰撞和感觉世界的前提下完善 Three.js 显示材质、灯光、阴影、茶几、糖果、花盆、
  叶片与花朵；保留明确的小室内尺度。
- 改果蝇头胸腹、复眼、足和翅的显示材质与层次；物理 mesh、质量、惯量和 actuator 不变。
- 解决 30 Hz pose 对约 218 Hz 拍翼的视觉混叠：浏览器以 snapshot 中的实际 flight activation、
  phase/frequency/envelope 生成显示专用翼姿或透明翼弧。它不反馈 native retina/物理，也不
  声称逐拍重建真实气动力。
- 地面/休息/清洁时翼幅应收拢；起飞和转向时左右翼显示响应实际神经/控制读出。

### 硬验收门槛

- 美术前后物理模型的 body/joint/DOF/actuator/sensor、质量、惯量、碰撞 geom 参数哈希完全一致；
  native retina 输入路径及阶段 5 固定输入行为结果不变。
- 自动 gallery 至少包含 room、table+sugar、plant+flower、grounded fly、walking、flight、
  feeding、grooming 和左右 native retina；所有图像非空且目标对象在画面内。
- 浏览器 wing fixture：非飞行幅度不超过飞行幅度的 10%；飞行时相位连续、左右转向幅差方向
  与 steering 一致；60 FPS 采样中无由 30 Hz pose 直接混叠产生的反向/冻结拍翼序列。
- 阴影保持开启且不产生此前高速运动重影；WebSocket 仍为 25–30 Hz，双眼预览 10–15 Hz。
- Windows viewer 默认 60 FPS；若自动环境无法读取 Windows RAF，沿用用户可见验收并生成
  可打开的最终 gallery/page，不虚构测值。full CNS + viewer realtime 平均不低于 1.00×。
- 阶段 4/5 的三种 300 秒固定种子回归门控、糖/花蜜取食、落地休息、爬行、防循环和清洁
  全部继续通过。

通过后提交：`Polish room fly appearance and neural wing display`。

## 9. 完成定义、失败处理与提交纪律

- 阶段失败时，只修复该阶段或调整实现；不得事后放宽数值门槛。若门槛被证据证明定义错误，
  先提交一份独立勘误说明和原因，再改门槛，不能与让结果“通过”的代码放在同一提交。
- 每阶段报告必须列出命令、版本、配置/hash、原始输出路径、通过/失败和已知局限。
- 每阶段通过后显式 stage 相关文件、检查 staged diff、提交并记录 commit hash；不 push。
- 不把诊断成功写成行为成功，不把工程调制写成已恢复的生物回路，不要求 CUDA 与 CPU-f64
  长程逐神经元完全一致。
- 六个阶段全部通过、六个阶段提交均存在、最终文档状态同步且工作树只剩明确排除的本地
  生成输出时，本轮目标才算完成。
