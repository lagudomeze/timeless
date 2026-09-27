# 目标架构：边界画在哪

> 🚧 **整篇是目标设计，代码里还没有。** 本篇回答一个问题：
> **切开来之后，谁裁决什么、谁住在哪、哪些东西必须删掉。**
> 两侧怎么说话在 [02-contract.md](02-contract.md)；怎么一步步迁过去在
> [04-migration.md](04-migration.md)。
>
> 术语：**模拟端**（sim，Bevy，无头，唯一权威）· **客户端**（client，Godot，
> 只画与只转发）· **表现契约**（两侧的接口，见 02）。

## 一、一句话结论

> **Bevy 退成"无头权威模拟"（headless authoritative simulation），
> Godot 成为唯一窗口与唯一表现层；两侧的接缝是「设备事件 + 姿态上行 /
> 数据视图下行」的窄接口，不是"把渲染拆一半"。**

这条接缝之所以能画得很窄，是因为本项目**已经**具备两个前提：

1. **逻辑整机可以无头运行**——这不是设想，是仓库里**已经存在**的东西：
   `crate::test_support::headless_app()`（[`src/lib.rs`](../../src/lib.rs)）装齐
   「除渲染外」的全部领域 + `MinimalPlugins`，`src/lib.rs` 里几十条整机用例
   跑的就是它。**这次改造本质上是"把测试夹具变成产品入口"。**
2. **世界的绝大部分时间是冻结的**——等玩家决策 / 威胁逼近时
   `Time<Virtual>` 停表（[`src/clock/mod.rs`](../../src/clock/mod.rs)）。
   进程边界带来的那点延迟被这个设计**天然吃掉**：玩家按键时世界本来就停着等他，
   `0.2ms` 的本机往返或 `30ms` 的跨机往返，玩家感知不到差别。

⚠️ 但要先说清**这一刀切掉了什么**：Godot 拿走的不是"几个面板"，而是
`presentation`（相机 / 纸片 / 装饰 / 特效 / 威胁格 / HUD）+ `interaction::visual`
（高亮与预演）+ `voxel_render` 的材质与网格资产层 + `spawn` 里的视觉子节点组装。
按 `src/` 的目录数，这是**三个域的一半加上第四个域的全部**。
时间成本要按[迁移阶段](04-migration.md)一段一段算，别按"换个 UI 框架"估。

## 二、为什么这个项目特别适合切

| 项目特性 | 对"逻辑与表现分家"的意义 |
| :--- | :--- |
| 世界只在"有人需要决策"时冻结 | 延迟不敏感：玩家输入时世界停着；**这是最大的红利** |
| 每个动作自带 `windup` / `recovery` 与 `execute_at` | 表现不需要预测：客户端拿到一条行动就知道"它何时落地、忙到何时"，可以本地规划动画（时间轴色块现在就是这么画的） |
| 位置只有一份真相（`Transform`），`Cell` 只在停下时更新 | 客户端不需要"格子坐标 → 世界坐标"的另一套换算（`ground_position` 由模拟端给） |
| 单位外观是 2D 纸片 + 贴地阴影，不是骨骼动画 | 表现层没有"动画状态机与逻辑对齐"的难题 |
| 体素网格化是**纯计算**（`build_chunk_meshes` 不碰 ECS、有 12 条单测） | 网格可以由模拟端烘焙后传数组，Godot 侧不必重写贪婪合并与 AO |
| 表现层已被铁律约束成**只读** | "客户端永不裁决"这条新纪律是旧纪律的直接延续，不是新增负担 |

## 三、三种进程形态

三者的**数据形状是同一份**（就是 02 的契约）。区别只在"数据怎么过去"。

### 形态甲：gdext 宿主 + 全 Rust 客户端

Rust 用 [godot-rust/gdext](https://github.com/godot-rust/gdext) 写一个
`Node3D` 子类作为宿主：它持有（headless 的）Bevy `App`，在 Godot 的
`_process(delta)` 里 `app.update()`，然后**直接读 `World`**（同进程，没有序列化）
把节点属性写下去。

- ✅ 零 IPC、零协议、零版本漂移、零延迟；键位 / 输入域几乎不用改。
- ❌ **UI 胶水也要用 Rust 写**——而"写 UI 累"正是这次改造的动机。
  拿 `get_node_as::<Label>(...)` + `set_text(...)` 拼面板，并不比 Bevy UI 舒服多少。
- 结论：**只该用它做宿主，不该用它写界面。**

### 形态乙：gdext 宿主 + GDScript 表现 ⭐ **推荐**

同一个 Rust 宿主，但把**边界收到一个节点**上：

```text
🆕 SimHost（Rust / gdext，唯一的跨界节点，场景树里就它一个）
  ├─ 方法（GDScript 可调）：step(delta) / send_key(...) / send_intent(name, payload)
  ├─ 信号（GDScript 可接）：view_updated(frame: Dictionary)
  └─ 内部：Bevy App（MinimalPlugins + GamePlugin 的 headless 版本）

其余全部是普通 Godot 场景 + GDScript：
  World3D / 区块 MeshInstance3D / 单位 Sprite3D / 地面指示 / HUD（.tscn + 主题 + 胶水）
```

- ✅ 零 IPC（同进程直接调用），但**界面与胶水用 GDScript 写**——编辑器拖布局、
  主题、Tween、信号，全都回来了。
- ✅ Rust 侧只需要认识"一棵 Godot 节点树里有一个自己的节点"，
  `src/` 的领域层一行 Godot 类型都不碰（不变量 2 由 **Cargo 依赖图**保证：
  模拟端 package 的依赖树里没有 `godot` crate）。
- ✅ 契约仍然按**可序列化**的样子设计（跨边界的类型必须是
  `Dictionary` / `Array` / 数值 / 字符串），所以将来要拆进程时是**搬家**，不是重写。
- ⚠️ 代价：改 Rust 要重编译动态库；Godot 的 GDExtension 热重载是
  "能用但别指望"的档位（细节见 [03-client.md](03-client.md) 与
  [05-risks.md](05-risks.md)）。

#### 形态乙的两种宿主实现（落地时二选一）

| 选项 | 说明 | 取舍 |
| :--- | :--- | :--- |
| **甲：先评估 `godot-bevy`** | 社区已有的桥（[`bytemeadow/godot-bevy`](https://github.com/bytemeadow/godot-bevy)），版本栈与本项目**逐项对齐**（godot-bevy 0.12.x ↔ Bevy 0.19 ↔ godot-rust 0.5 ↔ Godot 4.6）：`#[bevy_app]` 宏 + autoload `Node` + 主线程边界 + 日志桥 + `catch_unwind` | 省掉"宿主怎么写"这一整块；但它**默认开着 `experimental-threads`**（gdext 官方自述"high risk of unsoundness"），且它会把 **Godot 场景树镜像成 ECS 实体**、理念是 *Godot for Content, Bevy for Logic*——**与本方案（Bevy 是唯一权威、Godot 只是被动视图）相反**，要用就得把镜像那一半关掉 |
| **乙：自己写薄宿主** | 约 200 行：`step()` / 设备事件投递 / 视图打包 / `view_updated` 信号，全部跑在主线程 | 可控、可只依赖 gdext（不必开 `experimental-threads`）；代价是要自己处理 panic 兜底与日志转发，这两块可以直接抄 `godot-bevy` 的做法 |

#### ⚠️ 一条必须在 spike 里定的语义：谁驱动 `app.update()`

`godot-bevy` 的做法是**拆 Bevy 的 `Main`**：`First` / `PreUpdate` / `FixedMain` 跑在
Godot 的 `_physics_process`，`Update` / `PostUpdate` / `Last` 跑在 `_process`，
并明确写"**生产环境从不调用 `app.update()`**"（手动 `update()` 只承认对测试合法）。

本项目的时钟语义与它不一样：`clock::process_pause_requests` 是**唯一**写
`Time<Virtual>` 的地方，整机测试用 `TimeUpdateStrategy::ManualDuration`
（**一次 `app.update()` = 一帧**），世界大部分时间冻着。所以：

- **推荐**：自己驱动——`_process` 里调**一次** `app.update()`，语义与整机测试一致，
  虚拟时间也不会被 Godot 的物理 / 渲染双时钟切开；
- **备选**：用 `godot-bevy` 的 split driver，但**必须重验冻结语义**
  （物理 tick 与渲染帧不同频时，`PauseRequest` 的"每帧断言"还成不成立）。
- 这条是 [04-migration.md](04-migration.md) P−1 spike 的必验项。

### 形态丙：双进程（模拟端独立 + IPC 客户端）

模拟端是独立进程（甚至可以是服务端），客户端用 TCP / UDP / 命名管道收发契约数据。

- ✅ 客户端语言随你（GDScript 最舒服）；崩溃隔离；模拟端可以独立跑 CI / 回放 /
  压测；联机是它的自然延伸。
- ✅ 客户端掉线/卡住不影响模拟（而形态乙里，Godot 卡住 = 世界卡住）。
- ❌ 要**真的实现一遍协议**：序列化、帧号、重连、版本协商、抓包调试工具……
  这部分工作量不小，而且是"迟早要写"而非"现在就需要"。
- 结论：**先不做，但别把路堵死**——形态乙的契约已经是它的形状了。

### 推荐与拍板

1. **默认走形态乙**（gdext 宿主 + GDScript 表现）。
2. **契约（02）按可序列化设计**，即使形态乙用 `Dictionary` 传递也不例外。
3. **形态丙留作退路**：当出现"模拟端要独立跑"的真实需求（联机 / 专用服务器 /
   无头批量模拟）时再拆，拆的时候只换 `SimHost` 的实现，右侧的 Godot 场景不动。
4. ⚠️ **拍板一条现在会分叉的决定**：`PanCamera` / `ZoomCamera` 是"客户端自己做"
   还是"上行给 Rust 做"。相机已经在客户端了，**建议客户端自己做**（`camera.rs`
   的跟随与夹取逻辑照抄过去），`presentation::camera` 随之删除。
   若选上行，就必须把相机夹取规则留在 Rust——两种都行，**别两边都改**。

## 四、域的去留表

一域一行。**留** = 代码基本不动；**拆** = 一部分留、一部分删/搬；**去** = 整个搬去 Godot。

| 域 | 去留 | 具体怎么办 |
| :--- | :--- | :--- |
| `world` | **留** | 纯数据域，本来就零渲染依赖。地形、区块、体素读写、存档不动 |
| `world` 的方块交互 | **留** | `BlockCommand` → `apply_block_command_system` 是逻辑；客户端只负责把"点/挖"翻译成消息（`B` / `V` 之后由 UI 提供） |
| `voxel_render` | **拆** | `meshing`（贪婪合并 + AO + `face_shade`，纯计算、有单测）**留**，输出从 `Mesh` 改成 🆕 普通数组结构；`materials`（程序生成贴图 / 材质 / `Assets<Mesh>` 挂载）与 `lighting` 的渲染侧**去** Godot |
| `movement` | **留** | `Cell` / `MoveGoal` / `Velocity` / 可行走性 / 移动·跳跃·翻滚·冲刺执行器全是逻辑 |
| `combat` | **留** | 8 个子域全留。⚠️ `combat::targeting`（`HitRadius` / `MeleeShape` 相交）是**裁决**，不是画面，不许搬走 |
| `skills` | **留** | 静态目录。握手时导出成 `Directory.abilities`（图标路径、标签、热键都在这里） |
| `equipment` | **留** | 槽位 / 物品 / 加成是逻辑；装备**面板**去 Godot |
| `timeline` | **留** | 决策槽 / 行动实体 / 撤销 / 后摇恢复，一个字不改 |
| `clock` | **留** | 冻结判据与唯一的 `Time<Virtual>` 写入点不变。客户端只是**读** `world_time` + `frozen` |
| `ai` | **留** | 敌人决策与表现无关 |
| `input` | **改来源** | 设备事件从 winit 换成"客户端投递的 Bevy 输入消息"；**键位表一行不改**。相机基改读镜像（02 第二节） |
| `interaction` | **拆** | `pointer.rs`（悬停格 / 点击语义 / 预演读数）**留**，`raycast.rs`（纯函数）**留**；`ui_capture.rs` **删**（改由客户端报 `PointerOverUi`）；`visual.rs` **去** Godot（Rust 只产出"哪些格要画"的数据） |
| `presentation` | **拆** | 留：`hud/*/model.rs`（读数语义与文案，全部纯函数单测）、`log.rs` 的文案拼装、`help.rs` 的 `HELP_LINES`、`hint.rs` 的优先级判据、以及 **`CameraRig`**（它留下来当**相机镜像**的落点，`input` 因此一行不用改）。去 Godot：`camera.rs`、`unit_sprite.rs`、`decoration.rs`、`effects.rs`、`threat_grid.rs`、`preload.rs`、`MainCamera`、`hud/*/scene.rs`、`hud/*/system.rs`、`hud/layout.rs` |
| `spawn` | **拆** | 组装"逻辑零件"（`Health` / `Velocity` / `DecisionSlot` / `Cell` / `InputDriven` / 🆕 `ViewId`）**留**；组装**视觉子节点**（精灵、阴影）的部分去 Godot（那本来就是 `presentation` 提供的零件） |
| `config` | **留** | `config/actions.ron` 仍是数值真相。前摇 / 后摇会随行动视图传到客户端，客户端不需要自己读配置 |
| `main.rs` | **改写** | 不再 `DefaultPlugins`（那会开窗、拉渲染）；改为装 `GamePlugin` 的无头宿主。`bevy_remote` / `bevy_brp_extras`（BRP 调试）**建议保留**——它是无头时代唯一还能"看见世界"的手段，价值比现在更高 |
| 🆕 `view` | **新增** | 视图导出：`ViewId` 分配、实体增删改的增量、读数打包（见 02 第五、六节） |
| 🆕 `host` | **新增** | 宿主薄壳：把 `App` 包成"可被外部 step"的对象 + 设备事件投递口。**它是唯一允许碰 Godot 类型的地方** |

### 被保护下来的东西（改造的验收标准之一）

| 资产 | 为什么必须活下来 |
| :--- | :--- |
| `src/lib.rs` 的整机用例（真实流水线顺序 + `headless_app`） | 它们是"逻辑没被改造弄坏"的唯一保证。改造后仍应全绿，且**新增**整机用例覆盖"宿主 step 一次 = 世界走一帧" |
| `hud/*/model.rs` 的纯函数测试 | `ready`/`busy`、排序、截断、`FOCUS -`、溢出计数……这批 bug 的守门测试（[`docs/backlog/hud.md`](../backlog/hud.md)） |
| `combat/formula/domain.rs` 等零 Bevy 的公式 | 与本次改造无关，但要确认它们仍能在 `cargo test` 里单独跑 |
| `tests/assets.rs`（字体覆盖汉字 / 图标是带 alpha 的方图） | 文案生成仍在 Rust，所以这个验收仍然有效——**只要字体与图标两侧共用同一份文件** |
| `input/keyboard.rs` 与 `HELP_LINES` 的对账测试 | 键位真相仍在一处 |

## 五、Rust 侧新增的三块（以及它们的大小）

```text
src/
├── host/        🆕 宿主薄壳（唯一碰 Godot 的层；形态丙下换成 IPC 实现）
│   ├── step.rs        外部驱动的 app.update()
│   ├── devices.rs     设备事件 → Bevy 输入消息
│   └── pose.rs        相机姿态 / 光标 / PointerOverUi 镜像
├── view/        🆕 视图导出（增量 + 读数打包）
│   ├── id.rs          ViewId 分配与实体绑定表
│   ├── world_view.rs  单位 / 投射物 / 区块 / 地面指示
│   ├── hud_view.rs    读数视图（复用 hud/*/model.rs 的产出）
│   └── events.rs      Spawned / Despawned / HitEffect / LogLine / Hint
└── main.rs      改写：无头入口
```

设计纪律：

- **`host` 与 `view` 都不许反向依赖**：领域层不知道它们存在（同"没有任何域依赖
  `spawn`"那条铁律）。方向是 `host/view ──▶ 各领域`。
- **`view` 只读**：它不许改任何游戏状态（同"表现层只读"）。
- **跨边界类型必须是"可序列化形状"**：`host` 里允许有 Godot 类型的适配，
  但 `view` 产出的结构体只用 `f32` / `i32` / `String` / `Vec` / 枚举
  （未来拆进程时不用改它）。

## 六、与现有铁律的关系

| 铁律（[`docs/domain.md`](../domain.md) 第五节） | 改造后 |
| :--- | :--- |
| 1. `world` 零渲染依赖 | 不变（而且更重要了） |
| 2. UI 输入只翻译、不执行 | **升级**：客户端连"翻译"都只做一半——它只转发设备事实，语义翻译仍在 `input` |
| 3. 组件写入者唯一 | 不变 |
| 4. 消息定义在消费方 | 不变 |
| 5. **表现层只读** | **升级为"客户端永不裁决"**（[02](02-contract.md) 第九节不变量 1） |
| 6. 组装单向依赖 | 不变；视觉零件的组装搬去 Godot，`spawn` 只剩下逻辑零件 |
| 7. 物理附着 vs 逻辑关系 | 不变；客户端上的节点层级是**表现**，与 Bevy 的关系模型无关 |
| 8. 行动实体化 | 不变；行动实体是**视图**里的一等公民（时间轴色块要它） |
| 9. 冻结判据与唯一时钟写入点 | 不变；客户端只读 `world_time` / `frozen`（02 第七节的两套时钟） |
| 10. 执行器自己收尾 | 不变 |
| 11. 领域层零 Bevy | 不变，且新增一条同形的：**领域层零 Godot** |
| 12. 文档防漂移 | 新增两份契约文件 + 四侧对账测试（02 第八节） |

**新增两条铁律候选**（落地时写进 `AGENTS.md`）：

- **领域层零 Godot**：`src/` 的任何领域不许出现 `godot` crate 的类型；
  只有 🆕 `host` 可以。这条由 Cargo 依赖图强制，不靠自觉。
- **跨边界只有两种数据**：设备事实与意图（上行）、数据视图（下行）。
  出现第三种（客户端直接改状态、模拟端下发显示指令）就是架构坏了。
