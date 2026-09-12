# 阶段 1：small-room-v1 场景骨架结果

状态：**通过**。本文件只记录阶段 1 已实现和已复核的结果；阶段 2 以后仍以
`autonomous-fly-roadmap-2026-09-13.md` 为计划基准。

## 实现结果

- 新增版本化 `flybrain-scene-layout-v1` 描述，不复制或修改 MaleCNS、NeuroMechFly 身体网格。
- `--scene legacy` 保留原场景；`--scene small-room-v1` 在加载 `fly.xml` 后、编译 MuJoCo
  model 前应用静态 world-body 几何变更。
- 新房间为 `320 × 240 × 150 mm`，包含地面、封闭墙/顶、茶几、糖果碟、糖、盆栽花、
  花托和独立花蜜资源。
- 糖使用 `sugar_drop` / `food_patch`；花蜜使用 `flower_nectar` / `resource_nectar`，
  两套感觉身份没有复用。
- 场景描述、房间范围、活动几何、资源中心与 SHA-256 被输出到 `inspect` 报告。

## 门控证据

- 场景描述 SHA-256：`d763b7afb68ee528ec8f36cf93ab4c1958ad3488347d8cc1f648684e60834810`。
- `inspect --scene small-room-v1`：`qpos=133`、`dofs=132`、`bodies=71`、
  `joints=127`、`actuators=56`、`sensors=6`、`cameras=4`。
- 自动测试逐项确认所有活动场景 geom 存在且中心位于新房间范围；`food_patch` 与
  `resource_nectar` 的编译后中心分别精确匹配 `[45,15,32.5]` 和 `[-65,42,46.2] mm`。
- release 运行 `web-view --scene small-room-v1 --no-brain --speed 1000 --max-seconds 1`
  正常到达 1.0 生物秒并退出，无 MuJoCo warning、NaN 或 reset。
- legacy 加载路径仍不应用 overlay；相关单元测试保留原始计数和食物中心断言。
- `cargo clippy --features cuda --bin flybrain-world -- -D warnings` 通过。完整 lib suite 为
  259 passed / 3 failed；三个失败均因仓库未安装旧 `outputs/packs/flywire_v783` 测试资产，
  与本阶段代码无关，MaleCNS v1 pack 和本阶段定向测试均可用。

## 复现命令

```bash
cargo run --features cuda --bin flybrain-world -- inspect --scene small-room-v1
cargo test --features cuda small_room_scene_preserves_body_contract_and_resource_alignment --lib
cargo run --release --features cuda --bin flybrain-world -- web-view \
  --scene small-room-v1 --no-brain --speed 1000 --max-seconds 1 --bind 127.0.0.1:8876
```
