# indoor-v2 阶段2：双源气味、接触及感觉一致性

日期：2026-09-13。状态：阶段2通过。

## 修改及边界

- 两个源共用原有食物受体编码，释放强度均175 ppm、衰减长度35 mm，环境气流为零。
  使用现有advection-diffusion公式的零风特例（三维径向衰减），没有新建目标坐标导航器。
- 资源增加显式 `enabled` 实验开关，默认true；关闭后相应气味及味觉贡献均消失，几何不移动。
  这是实验消融接口，不是浏览器或控制器获得真实食物坐标的接口。
- 新版两源接触半径3 mm，并增加相对中心高度±0.75 mm的薄层限制；原场景不配置该限制，
  维持原有语义。薄食物上方1 mm不再误获得味觉。有效进食仍另外需要支撑、口器、MN9和
  饥饿下降，阶段5再完整验证，不能以本阶段taste ID测试充当寻食成功。
- Scene出生位置进入共同SimulationStepper初始化和重置；viewer及cns-check均不再把
  indoor-v2糖源移到前方。旧场景的交互放糖兼容保留。
- reset恢复新场景出生点与原糖位置；完整CNS/需求/随机/暂停协议另在阶段3验收。

## 已验证的结果

1. 固定采样点双源严格等于两个单源之和（误差<1e-12）；分别禁用任一资源，另一源保持；
   零源时浓度为0。单源沿高度方向0/2/5/10/20/40/80 mm严格递减。
2. 下列浓度经原OlfactoryTransducer编码均有限、有界，配对偏移 `[1,0.5,1]` mm仍产生
   >1e-5的感知差异，不出现双源叠加直接饱和到无梯度。

| 采样位置 (mm) | 总浓度 ppm | 糖贡献 | 花贡献 |
|---|---:|---:|---:|
| [26,-12,32.1] | 3.631848 | 3.622057 | 0.009790 |
| [0,0,40] | 0.325492 | 0.201315 | 0.124178 |
| [-48,12,45] | 4.415146 | 0.002691 | 4.412455 |
| [48,-12,50] | 4.990368 | 4.987710 | 0.002658 |

3. 单源中心/2.9 mm水平内侧为对应taste ID，3.1 mm外侧、薄层上方及盆底不是接触；
   SimulationStepper身体口器位置fixture验证进入/离开立即刷新，控制窗口2 ms，无20 ms延迟队列。
   初始近源没有taste；改变糖位置后reset恢复权威位置和出生点。
4. 全CNS新场景web-view正常运行60仿真秒：166700 neurons，NVIDIA GeForce RTX 5080；
   native隐藏OpenGL为D3D12/RTX 5080，共交付857次retina更新。此次不是正式FPS/实时率验收。
5. `examples/indoor_retina_probe.rs` 通过native隐藏传感器固定两个身体姿态采样，不改真实仿真
   时间；诊断自己的MjData时标仅用于PBO采样排程。左右顺序沿用具名left/right camera ID和
   左右顺序拼接，输出双眼图及摘要，已亲自查看。糖果位于身体前方22 mm；花蜜前方12 mm。
   图像是721小眼的灰度预览，不是糖果/花蜜识别结果，不恢复组胺通路。

| 固定姿态 | 左均值 / 对比度 | 右均值 / 对比度 |
|---|---|---|
| 糖果观察点 | 0.376557 / 0.097015 | 0.386996 / 0.096721 |
| 花蜜观察点 | 0.419473 / 0.143003 | 0.388985 / 0.138688 |

native几何、位置和材质基色与Three.js同源；浏览器程序表面纹理没有进入native retina，
不宣称两种渲染器逐像素匹配。固定图见 `outputs/indoor-v2/stage-2/fixed-retina/`，运行中
WebSocket预览见 `outputs/indoor-v2/stage-2/retina/`。

诊断失败记录：最初把OpenGL姿态探针放在Rust测试线程，采样完成后退出SIGSEGV；改为GLFW
所需主线程的独立example后正常退出，未改生产retina路径。失败测试不计通过。

## 验证命令及资源

```sh
python3 tools/test_indoor_v2.py
cargo test --release --features cuda --lib indoor_v2 -- --nocapture
cargo test --release --features cuda --lib habitat::tests -- --nocapture
cargo run --release --features cuda --example indoor_retina_probe
```

新habitat SHA256：`5cc8663bebbbfc308ee0be383f65496bf98daba21e4a608ca0e237ad8bebac40`。
XML与场景JSON哈希仍与阶段1相同。MaleCNS四个 `.npy` 实际文件SHA256逐一与manifest相符；
神经元166700、边24469412，pack和神经绑定未修改。

最终回归：habitat 7项、indoor-v2 3项、单独初始化/重置1项均通过；Rust CUDA release构建、
Python静态契约3项、JavaScript语法、diff whitespace检查通过。

最终1秒CUDA `cns-check --scene indoor-v2` 冒烟确认默认出生 `[26,-12,32.1]`、全脑活动有效，
耗时0.819墙钟秒。也再次暴露待修行为：0.822秒处于飞行、速度峰值185.54 mm/s、没有进食。
这不是阶段4/5的通过证据；该短测包含工作区此前起降候选代码，不作为已提交行为基线。
