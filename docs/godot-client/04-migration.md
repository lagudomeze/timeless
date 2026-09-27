# 迁移路线：P0–P4 五段走完（外加一天的 spike）

> 🚧 **整篇是目标设计，代码里还没有。**
> 本篇回答一个问题：**怎么一步步搬过去，每段凭什么说做完了。**
> 边界在 [01-architecture.md](01-architecture.md)，两侧怎么说话在 [02-contract.md](02-contract.md)。
>
> 每段都有**退出标准**——没有证据不许往下走（同 [`TODO.md`](../../TODO.md) 的勾选规则）。

## 零之前（P−1）：动手之前先花一天做 spike

在承诺 P0 之前，先花**半天到一天**验掉最不确定的三件事。它们全都会一票否决或改写方案：

| 要验的 | 怎么验 | 不通过的后果 |
| :--- | :--- | :--- |
| gdext 能编译、能被 Godot 加载、能被 GDScript 调用 | 一个只有 `#[func] fn ping() -> String` 的类，挂在场景里打印出来（注意：Godot 里是**改节点类型**到你的类，不是挂脚本） | 形态乙不成立 → 退回形态丙（IPC + GDScript） |
| 宿主能驱动一个**空的** Bevy `App`（`MinimalPlugins`）跑 100 帧不崩 | `step()` 里 `app.update()`，同时写一帧日志 | "同进程双引擎"的假设破产 → 必须拆进程 |
| **`App`（含大量 `!Send` 资源）能不能放进 `Gd<T>` 用户实例** | 直接把 `App` 塞进 `#[derive(GodotClass)]` 的结构体，编译 + 跑 | 不行的绕过办法：`App` 放 `thread_local!` / 静态槽，节点只持有句柄 |
| **能不能不开 `experimental-threads`** | 只碰主线程的最小宿主 + Bevy 的任务池（网格化）一起跑 | 必须开的话就要接受官方口径的"high risk of unsoundness"（[05](05-risks.md) B7） |
| **驱动方式**：一次 `_process` = 一次 `app.update()` | 让世界跑一段，检查 `Time<Virtual>` 的推进与冻结断言（`PauseRequest` 是每帧断言）是否符合现在的语义 | 不合就改用 `godot-bevy` 的 split driver，但**要重验冻结语义**（[01](01-architecture.md) 形态乙） |
| **先评估 `godot-bevy` 能不能当宿主**（0.12.x ↔ Bevy 0.19 ↔ godot 0.5 ↔ Godot 4.6） | 读它的 Timing / Threading 章，看它的场景镜像与 split driver 能不能按要求退场 | 不能退场就自己写薄壳（约 200 行），panic / 日志那两块照抄它的做法 |
| Rust 侧 `panic` 不会带走编辑器（或能被兜住） | 故意 panic 一次；再故意在 `Drop` 里 panic 一次（后者官方口径是 **abort**） | 日常开发会被"崩编辑器"折磨（[05](05-risks.md) B7） |

⚠️ **这个 spike 的代码不进主干**（它只是验证），但**结论要写进本文档**（哪一条被证伪、
改成了什么）。参考本仓库的既有做法：文档里"已验证"与"未验证"必须分得开
（[`docs/index.md`](../index.md) 的 🚧 规矩）。

## 零、贯穿始终的护栏：双跑对照

迁移期最大的风险不是"搬不过去"，而是**搬了一半之后没有东西能证明搬对了**。
所以先钉两条：

1. **Bevy 窗口不删**（P0–P3 期间它是**参照实现** / golden reference）。
   两套表现并排跑，任何"读数不一样"都是 bug，而不是"我觉得应该是这样"。
   它在 **P4** 才被删掉。
2. **第二件事就是让 `view` 能离线导出快照**：🆕 `cargo run -- --headless
   --dump-view target/view-0001.json`，把某一刻的世界 + 读数落成一个 JSON。
   这一份快照有三个用途：
   - **Godot 侧可以脱离模拟端开发**（像喂回放一样喂它）：P1 的地形与单位渲染、
     P2 的每一块 HUD，都可以先在快照上做出来；
   - 它是 [02](02-contract.md) 第八节那份**契约测试的夹具**（字段路径表就从它长出来）；
   - 它是回归比对的锚点：同一份快照，两侧渲染出的读数必须逐字符相同。

⚠️ 这条护栏顺带解决了一个心理问题：**每一段做完都应该"看得见"**，
没有"连着两周屏幕上什么都没有"的阶段。

## 一、P0 骨架：没有画面，但一切都能被验证

**目标**：模拟端能无头跑、能导出视图；契约文件与 Rust 侧对账测试到位。

| 动作 | 备注 |
| :--- | :--- |
| 新增 🆕 `src/view/`（`ViewId` 分配 + 实体增量 + 读数打包） | 见 [01](01-architecture.md) 第五节 |
| 新增 🆕 `src/host/`（`step()` / 设备事件投递 / 姿态镜像） | P0 只需能在测试里驱动 |
| `main.rs` 支持无头入口与 `--dump-view` | 仍然保留开窗那条路（双跑对照要用） |
| 单位 / 投射物 / 区块挂 🆕 `ViewId` | 加一条测试：每个视图可见实体都有唯一 `ViewId`，销毁时被回收登记 |
| 写 `contract/*.json` + Rust 侧三条对账测试 | 见 [02](02-contract.md) 第八节 |
| 新增依赖：`client/rust/` 的 `godot`（0.5，MPL-2.0）与可能的 `godot-bevy`（0.12） | 按仓库规矩**手动写入 `Cargo.toml`**（不用 `cargo add`）并登记进 [`TODO.md`](../../TODO.md) 的「依赖与文档索引」 |
| 一条整机用例：**宿主 step 一次 = 世界走一帧** | 改造后最重要的回归锚点（时间轴仍按真实流水线顺序） |

**退出标准**

- `cargo test` 全绿（新增的对账测试**删掉就红**）；
- `cargo run -- --headless --dump-view target/view-0001.json` 产出的快照里能读到：
  玩家与敌人的位置 / 阵营 / 决策槽、区块网格（顶点数对得上）、
  时间轴读数（状态行 + 车道色块）、单位面板读数、日志条目；
- BRP 仍可用（`brp_status` 能连上，`world.query` 能读到 `PauseReasons`）。

## 二、P1 Godot 侧骨架 + 世界可见（只读）

**目标**：Godot 窗口里出现与 Bevy 窗口**同一片地形、同一批单位**。

| 动作 | 备注 |
| :--- | :--- |
| 建 Godot 项目（`client/`）与 gdext 宿主节点 | 见 [03-client.md](03-client.md) 的目录约定与 `.gdextension` |
| 实现 🆕 `SimHost`：`step(delta)` + `view_updated(frame)` | 形态乙，见 [01](01-architecture.md) 第三节 |
| 先做**快照加载器**：能读 `--dump-view` 的文件并渲染 | 客户端可以脱离模拟端开发 |
| 地形：区块网格 → `ArrayMesh`（材质 + 顶点色） | 贪婪合并与 AO 仍在 Rust（[01](01-architecture.md) 第四节）。⚠️ **上传时反转三角形绕序**（Godot 正面是顺时针，Rust 生成的是逆时针），先拿一个孤立方块验六面可见（[05](05-risks.md) C2） |
| 单位：纸片 + 贴地阴影 + 相机（正交 / 斜视角，与现有 `CameraRig` 对齐） | `assets/textures/units/*.png` 两侧共用 |
| 装饰：`assets/models/nature/*.glb` 按 Rust 给的摆放表实例化 | 摆放表是数据，视觉是客户端的事 |

**退出标准**

- 固定机位截图：Godot 版与 Bevy 版并排，**地形轮廓、单位位置、明暗（AO）一致**；
- 世界冻结时（玩家空闲）两侧都静止，且**没有一帧的漂移**（连续截 10 帧做差）；
- `client/` 能在 `--headless` 下加载场景不报错（为将来的 Godot 侧测试铺路）。

## 三、P2 HUD 逐块搬家（一次一块，块块可验收）

**目标**：`presentation/hud/` 下**只剩下 `model.rs` 一族**（读数语义），
`scene.rs` / `system.rs` / `layout.rs` 全部删除。

搬家的顺序按"依赖少的先搬、玩家最常看的先搬"：

| 顺序 | 块 | Rust 侧留下什么 | Godot 侧做什么 |
| :--- | :--- | :--- | :--- |
| 1 | **时间轴**（含候场区、悬停读数） | `hud/timeline/model.rs` + `readout.rs` | 车道 / 色块池 / 刻线 / 候场区 / tooltip |
| 2 | **单位面板**（玩家格 + 敌人行池 + 溢出计数） | `hud/panels/model.rs` | 行池、血条 / 精力条、Focus 三点、洞察力行 |
| 3 | **技能栏** | `hud/skills/model.rs` + `combat::attack::MenuSelection` | 图标按钮、四种状态样式、tooltip |
| 4 | **战斗日志** | `log.rs` 的文案拼装 | 可折叠面板、时刻前缀、滚动 |
| 5 | **提示条 + 帮助** | `hint.rs` 的优先级判据、`help.rs` 的 `HELP_LINES` | 淡出（用 `real_delta`）、`F1` 面板 |

**逐块的完成判据（三条同时满足）**

1. Rust 侧那块的 `model.rs` 测试**全绿且一条没删**（`ready`/`busy`、排序、截断、
   `FOCUS -`、溢出计数…… 见 [`docs/backlog/hud.md`](../backlog/hud.md)）；
2. **两侧读数逐字符一致**：同一份视图数据，Bevy 版与 Godot 版渲染出的那行字完全相同
   （比字符串，不比截图——截图对不上时你分不清是语义错还是字体错）；
3. [`playtest-checklist.md`](../playtest-checklist.md) 里属于这块的条目在 Godot 窗口里过一遍。

⚠️ **两套 HUD 并存期的纪律**：任何 HUD 改动**只改 `model.rs`**，两侧都从它读。
谁要是"顺手在 Bevy 版里补一句文案"，两侧立刻分叉，而这正是本设计最贵的 bug。

**退出标准**

- `ls src/presentation/hud/**` 里只剩 `model.rs`（+ `mod.rs` 门面）；
- `presentation/hud/layout.rs` 那张"具名节点清单"（40+ 个名字、
  `setup_hud_names_every_region_it_builds`）被**等价物**取代：
  Godot 侧场景的节点名清单 + 契约文件的对账测试（[02](02-contract.md) 第八节）。
  ⚠️ 这份清单不能就这么丢掉——它是"HUD 有哪几块"的**验收表**，
  搬过去以后它应该变成 `client/` 里的一份**场景契约测试**。

## 四、P3 输入与交互切换

**目标**：玩家在 Godot 窗口里完成全部操作，Bevy 窗口的输入被关掉。

| 动作 | 备注 |
| :--- | :--- |
| 设备事件转发（键 / 鼠标 / 滚轮 / 窗口尺寸） | 02 第一节；事件必须在 `step()` 之前投递完 |
| 相机镜像 + 光标 + `PointerOverUi` | 02 第二节；`interaction::ui_capture` 随之删除 |
| 拾取：客户端给射线 / 坐标，Rust 算格（`pick_cell` 保留） | 悬停格、预演读数、`B`/`V` 方块交互都靠它 |
| 地面指示：悬停格 / 威胁格 / AOE / 扇形预演 → 格集合下行 | `interaction::visual.rs` 拆除；判定（`MeleeShape` 等）留在 Rust |
| 特效：命中粒子、日志行、提示条 → 事件下行 | 粒子用 `world_time`（冻结时定格），提示条用 `real_delta` |
| UI 意图上行（技能栏点击、暂停按钮、帮助、重开……） | 02 第三节；顺手把"三连消息"收口成一个构造函数 |

⚠️ **切换点必须是一次性的**：不要"两边都能收输入"（玩家会发现按一次走两格，
而排查时你以为在看同一个世界）。切换那天起，Bevy 窗口只作为**只读观察窗**。

⚠️ **BRP 调试流程要更新**（它是当前最主要的实机手段）：

| 现有手段 | 拆开之后 |
| :--- | :--- |
| `world.query` / `world.get_resources`（读状态） | **照旧可用**（模拟端还开着 BRP），而且价值更高——它是唯一能直接看世界的地方 |
| `brp_extras_send_keys`（注入输入） | **照旧可用**（注入的还是 Bevy 的 `ButtonInput`），但要注意"每个键只按一次"那条坑仍然成立 |
| `brp_extras_screenshot`（截图） | **失效**（模拟端不再渲染）→ 换成 Godot 侧 `get_viewport().get_texture().get_image().save_png()`，写进实机清单 |

**退出标准**

- [`playtest-checklist.md`](../playtest-checklist.md) 的 1–6 节**全部在 Godot 窗口里**跑完
  （含"UI 穿透"一节：点每个面板时世界不动、`HoveredCell` 变 `None`、预演消失）；
- 三种冻结原因都显示正确（`awaiting` / `threat` / `manual` 叠加，含 #50 那条）；
- Godot 侧截图脚本能在固定机位出图，与 Bevy 版对照过一轮。

## 五、P4 拆干净

**目标**：模拟端里再也找不到"画"这个字；`cargo tree` 里没有渲染依赖。

| 动作 | 备注 |
| :--- | :--- |
| 删 `presentation/` 的视觉部分：`camera.rs` / `unit_sprite.rs` / `decoration.rs` / `effects.rs` / `threat_grid.rs` / `preload.rs` / `MainCamera` / `plugin.rs` 的相应注册 | 保留 `hud/*/model.rs`、`log.rs` 文案、`help.rs`、`hint.rs` 判据，以及 **`CameraRig`**（相机镜像的落点，[02](02-contract.md) 第二节）——`input` 因此一行不改 |
| 删 `interaction/visual.rs` / `ui_capture.rs` | `pointer.rs` / `raycast.rs` / `components.rs` 留着 |
| 拆 `voxel_render` | 🚧 **待拍板**：① 改名 🆕 `meshing`（保留独立域：它有异步任务池与脏标记的生命周期）；② 并进 `world` 当子域（`world` 必须零渲染依赖——只产数组是满足的）。**建议 ①**，理由是"派生数据 ≠ 体素数据"，且 ② 会把 `world` 撑胖 |
| `spawn` 里摘掉视觉子节点组装 | 只组装逻辑零件 + `ViewId` |
| `main.rs` 去掉 `DefaultPlugins` | 换成 `MinimalPlugins` + 需要的领域插件（`headless_app()` 已经是一份可用的清单） |
| `Cargo.toml` 瘦身 | ⚠️ **等删干净再关 features**，否则会撞上一大堆编译错误。目标：保留 `bevy_ecs` / `bevy_app` / `bevy_time` / `bevy_input` / `bevy_asset` / `bevy_scene`（BSN 要用）/ `bevy_log` / `bevy_remote`（调试）/ `bevy_transform`；去掉 `bevy_render` / `bevy_pbr` / `bevy_ui` / `bevy_window` / `bevy_winit` / `bevy_sprite` / `bevy_text` / `bevy_gltf` / `bevy_audio` / `bevy_animation` |
| `hot-reload` feature 拆分 | 它现在有两半：`bevy/file_watcher`（资产热重载 → 不再需要，资产是 Godot 的事）与 `notify-debouncer-full`（`config/actions.ron` 热重载 → **保留**，它是 Rust 侧数值） |
| 文档同步 | `docs/index.md` 文档地图、`docs/domain.md` 域地图与铁律、`AGENTS.md` 的「模块约定」+ 新增两条铁律（[01](01-architecture.md) 第六节） |

**退出标准**

- `cargo test` / `cargo clippy --all-targets -- -D warnings` / `cargo fmt --check` 全绿；
- `cargo tree -p app | Select-String bevy_render` **无输出**（渲染依赖真的没了）；
- 无头启动不再需要 GPU（在没显卡的 CI / 容器里能跑完一整局整机测试）；
- `AGENTS.md` 的验收命令更新（`cargo run` 变成"启动模拟端 + 启动客户端"两步）。

## 六、可选 P5：拆进程 / 联机

**触发条件**（有其一才做）：需要专用服务器 / 联机 / 无头批量模拟与回放 /
客户端崩溃不许带走世界。

**要做的**：把 🆕 `host` 的实现从"同进程调用"换成"IPC 收发"，两侧的数据形状不动
（这就是 [02](02-contract.md) 要求"跨边界类型必须是可序列化形状"的原因）。
需要新增的只有：序列化、连接生命周期、协议版本协商、以及一个**协议抓包/回放工具**
（没有它，跨进程的 bug 会非常难查）。

**别提前做的**：重连、预测与回滚、兴趣管理（entity culling）。这个游戏的
"世界会停下来等你"已经解决了延迟问题，**不需要**客户端预测。

## 七、工作量与风险的诚实估计

**量级（数得出来的部分）**

| 区域 | 源文件数 | 单测数（`grep -c '#\[test\]'`） | 归宿 |
| :--- | ---: | ---: | :--- |
| `presentation/` | 29 | 112 | 约一半是**读数语义与文案**（`model.rs` 一族 / `log.rs` / `help.rs` / `hint.rs` 判据 / `readout.rs` ≈ 59 条）→ **原地存活**；其余（相机 / 纸片 / 特效 / 威胁格 / 各 `scene.rs` / `system.rs` / `layout.rs` ≈ 53 条）随实现删除 |
| `interaction/` | 8 | 19 | `pointer.rs` + `raycast.rs`（11 条）存活；`visual.rs` + `ui_capture.rs`（8 条）删除 |
| `voxel_render/` | 14 | 26 | `meshing/utils.rs`（16 条）存活；`materials/*` + `lighting/systems.rs` 删除 |
| `spawn/` | 7 | — | 骨架存活，视觉子节点组装搬走 |
| 合计被碰到的文件 | **≈ 58 / 167**（约 35%） | ≈ 157 中约 **86 条存活** | 存活率 ≈ 55% |

> 这张表的意义：**这次改造不是"重写游戏"，而是"重写表现层"**，
> 而且表现层里最值钱的那一半（读数语义、文案、拾取、网格化）是**带走**的，不是重写的。
>
> ⚠️ 数字是"文件与测试"的量级，不是工时。真正的工时大头在 P1（地形与单位的渲染）
> 与 P2 的逐块搬家上——它们没有存量代码可抄，是**新写**的一份。

**三条最容易翻车的地方**（详细清单见 [05-risks.md](05-risks.md)）

1. **P2 期间两套 HUD 分叉**：纪律是"只改 `model.rs`"，并由对账测试守。
2. **冻结时表现偷偷动**（用 `real_delta` 补 `world_time`）：这是"界面在说谎"的翻版，
   而且更难发现。判据表在 [02](02-contract.md) 第七节。
3. **资产与字体的双份真相**：一旦 Godot 侧"顺手改了一句文案"或"另存了一份字体"，
   `tests/assets.rs` 就失去意义了。

**别并行两条路**：迁移期内**不要**同时推进 Bevy HUD 的新功能与 Godot HUD 的搬家
（比如一边加"伤害数字"一边搬面板）。同一块 UI 在同一时间只能有一个作者，
否则两侧永远对不齐。
