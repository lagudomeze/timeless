# 架构与模块

> **描述对象：代码 A（仓库根 `src/`，package `app`，Bevy 0.19）。**
> 本文是模块结构、分层约束与执行顺序的权威说明，与代码同步维护。
> 配套阅读：[timeline.md](timeline.md)（无回合时间线的语义） ·
> [components.md](components.md)（组件 → 系统逐层对照）。

## 一、项目定位

**无回合**战斗时间线 + 戴森球式供应链 + 「信息即力量」的 roguelike 策略游戏。
战斗的节奏不来自回合，而来自三件事：

1. 每个单位自己的 `DecisionSlot`——**决策槽空着就能决策**；
2. 每个动作自带的**前摇 + 后摇**（`ScheduledAction` 的时间戳）；
3. **暂停原因集合**——玩家等输入、手动暂停、威胁逼近都只是往集合里加一条原因。

详见 [timeline.md](timeline.md)。

## 二、分层与铁律

| 层 | 载体 | 职责 | 约束 |
| :--- | :--- | :--- | :--- |
| 领域层 | 纯 Rust，零 Bevy | 防御判定、招架反制 | **零 Bevy 依赖**，可脱离渲染单测 |
| 应用层 | Bevy 组件 / 系统 / 插件 | 组件、系统、消息、渲染、HUD 编排 | **不含伤害公式** |
| 表现层 | 应用层内的 `presentation` / `voxel_render` | 相机、HUD、日志、体素网格 | **只读游戏状态**，只写表现 |

领域层在代码里的落点是 `src/combat/formula/domain.rs`：
`DefenseState` / `resolve_defense` / `counter_damage`——纯函数 + 纯数据。
实体身份用 `u64` 折值传进来，因此这一层完全不认识 Bevy。

其余铁律（每条都有代码里的对应物，改代码前先读这张表）：

| 铁律 | 含义 | 违反的后果 |
| :--- | :--- | :--- |
| 一个领域 = 一个目录 = 一个 `Plugin` | `mod.rs` 只做 `pub mod` + `pub use` 门面 | 领域边界消失，循环依赖 |
| 输入只翻译、不执行 | 键盘 / 鼠标只写消息，落盘由消费领域负责 | 输入系统变成上帝系统 |
| 消息与消费系统同域 | 新增消息在**消费方**插件 `build` 里 `add_message::<T>()` | 生产者与消费者绑死 |
| 行动实体化 | 行动 = 独立实体（载荷 + `ScheduledAction` + 可选的 `Uncancellable`），`add_child` 挂在行动者下 | 调度器开始认识载荷 |
| 暂停只用 `Time<Virtual>` | 唯一写时钟的是帧末的 `apply_clock`（原因集合 `PauseReasons`） | 各领域冒出 `if paused` 分支 |
| 状态写在决策槽里 | 前摇 / 后摇由 `DecisionSlot` 的三态回答，时刻由 `execute_at` 与 `until` 回答 | 标记与时间戳打架（重复触发 / 忘了摘） |
| 决策按格、结算按真实距离 | `Cell` 管决策，`Transform` 距离管命中 | 两套坐标各算一半，命中飘忽 |
| 组装单向依赖 | `spawn` 依赖所有领域，**没有任何领域依赖 `spawn`** | 改角色配置波及战斗规则 |
| 功能不是领域 | 只把已有系统拼一次的胶水留在调用方 | 每个功能都长出一个空领域 |

## 三、领域一览

| 领域 | 职责 | 插件 | 系统集 |
| :--- | :--- | :--- | :--- |
| `world` | 体素地图**数据**：区块、地形生成、体素读写（**零渲染依赖**） | `WorldPlugin` | `WorldSet` |
| `voxel_render` | 体素**表现**：异步网格化、材质、明暗 | `VoxelRenderPlugin` | `VoxelRenderSet` |
| `movement` | 格子决策 + 速度位移 + 移动 / 跳跃 / 翻滚行动载荷 | `MovementPlugin` | `MovementSet` |
| `combat` | 生命 / 伤害 / 目标获取 / 攻击生命周期 / 技能 / 精力 / 防御 / 威胁检测 | `CombatPlugin` | `CombatSet` |
| `timeline` | **无回合**调度：谁能决策、行动何时到点、后摇何时结束、暂停原因 | `TimelinePlugin` | `TimelineSet`、`ClockSet` |
| `ai` | 敌人决策（选意图 + 声明行动，不碰规则） | `AiPlugin` | `AiSet` |
| `input` | 玩家输入源：键盘 / 鼠标 → 消息（**只翻译**） | `InputPlugin` | `InputSet` |
| `interaction` | 鼠标交互：射线拾取悬停格 → 高亮 / 预演；点击 → 各领域的消息 | `InteractionPlugin` | `InteractionSet` |
| `presentation` | 表现：相机 / 单位纸片与贴地阴影 / 装饰 / 战斗日志 / HUD | `PresentationPlugin` | `PreloadSet`(Startup)、`PresentationSet`(Update) |
| `spawn` | **组装车间**：把各域零件拼成角色实体；含开局组装与重置 | `SpawnPlugin` | `AssemblySet`(Startup)、`SpawnSet`(Update) |

## 四、目录结构

```text
src/
├── main.rs                     # 只加引擎插件 + GamePlugin
├── lib.rs                      # GamePlugin / configure_pipeline / 整机集成测试
├── world/                      # 体素地图数据（纯数据，零渲染依赖）
│   ├── plugin.rs               #   WorldPlugin
│   ├── voxel/{components,types}.rs
│   ├── chunk/{components,events,systems}.rs
│   ├── terrain/{resources,systems}.rs      # 噪声地形纯函数 + TerrainConfig
│   └── storage/{resources,systems}.rs      # ChunkMap + get_voxel / set_voxel
├── voxel_render/               # 体素表现
│   ├── meshing/{components,resources,systems,utils}.rs   # 异步面剔除网格化
│   ├── materials/{assets,resources}.rs                   # 按类型分组的材质
│   └── lighting/systems.rs                               # 面朝向明暗
├── movement/
│   ├── cell.rs                 #   Cell / MoveGoal（决策层坐标）
│   ├── components.rs           #   Velocity / MoveSpeed
│   ├── events.rs               #   MoveCommand / MoveToCommand / JumpCommand
│   ├── actions.rs              #   MoveAction / JumpAction / RollAction + 工厂 + 声明/执行器
│   ├── systems.rs              #   move_entities_system / follow_terrain_system
│   └── plugin.rs
├── combat/
│   ├── components.rs           #   Faction / Collidable
│   ├── attributes/components.rs#   PhysicalDamage / Armor / HitRadius / AttackRange / AttackFrame / InterruptPower
│   ├── health/                 #   Health + DeathEvent + 扣血与死亡销毁
│   ├── targeting/              #   CollisionTarget / MeleeShape + 碰撞与扇形检测
│   ├── lifecycle/              #   Projectile / HitOnce / Lifetime + 清理
│   ├── formula/                #   domain(零 Bevy 防御逻辑) / systems(护甲 + 命中) / events(DamageEvent)
│   ├── defense/                #   Stamina / Dodging / Parrying / ParryAction + 翻滚与招架
│   ├── reaction/               #   Threatens / TargetCell / ThreatWindow + 威胁检测
│   ├── skills/                 #   registry / menu / melee / arrow / fireball / explosion / actions
│   └── plugin.rs               #   CombatPlugin（战斗流水线）
├── timeline/
│   ├── timing.rs               #   ActionTiming 的**形状**（具体数值归各领域的载荷）
│   ├── decision.rs             #   DecisionSlot 三态（Empty / Windup / Recovery）+ recovering()
│   ├── schedule.rs             #   ScheduledAction（这一手什么时候落地）
│   ├── components.rs           #   Uncancellable / InputDriven（行动实体与行动者的标记）
│   ├── resources.rs            #   PauseReasons / Focus / FocusIntent
│   ├── events.rs               #   PauseRequest / PlayerIntent / UseFocus / ActionBlocked / UndoCommand / ActionCancelled / DecisionReady
│   ├── systems.rs              #   暂停原因 / Focus / 撤销 / 后摇 / apply_clock
│   └── plugin.rs
├── ai/{components,systems,plugin}.rs
├── input/{keyboard,pointer,plugin}.rs   # 键盘 / 鼠标 → 消息（只翻译）
├── interaction/                # 拾取 / 高亮 / 预演 / 点击 → 消息
├── presentation/               # 相机 / 单位纸片 / 装饰 / 日志 / HUD
│   └── hud/{layout,panels,actions,skills,timeline,log_panel,hint,help}.rs
└── spawn/                      # 组装车间：unit / player / enemy / assembly / restart
```

## 五、跨领域执行顺序

顺序只在 `lib.rs::configure_pipeline` 里声明一次；**测试复用同一个入口**，
所以测试跑的就是真实的流水线顺序。

```text
Startup:  PreloadSet ─▶ AssemblySet
Update:   SpawnSet ─▶ InputSet ─▶ InteractionSet ─▶ TimelineSet ─▶ AiSet
          ─▶ MovementSet ─▶ CombatSet ─▶ VoxelRenderSet ─▶ PresentationSet ─▶ ClockSet
WorldSet ─────────────────────────▶（必须早于 VoxelRenderSet：数据先于表现）
```

读法：谁写的消息排在谁前面。例如 `InputSet` 写 `MoveCommand`，`MovementSet` 消费它；
`WorldSet` 生产区块数据，`VoxelRenderSet` 才网格化；`ClockSet` 排在帧末，
因此「冻结 / 解冻」永远只影响下一帧（一帧之内所有领域看到同一个时钟状态）。

各领域**内部**的顺序由自己的 `plugin.rs` 维护：

| 系统集 | 内部链 |
| :--- | :--- |
| `TimelineSet` | 断言空决策槽 → Focus 意图 → 打断（写撤销请求）→ 撤销 → 后摇恢复 → Focus 回复 |
| `ClockSet`（帧末） | 暂停请求（每帧重建原因集合）→ `apply_clock`（**唯一**写 `Time<Virtual>`） |
| `InputSet` | 方向键 / 技能热键 / 技能栏 / 释放 → 消息；空格 → `PauseRequest`；`F5` → `ResetBattle`；`F1` → `ToggleHelp`；Shift + 决策键 → `UseFocus`；相机平移 / 缩放；左 / 右键 → `PointerCommand` |
| `MovementSet` | 声明（移动/点地/跳跃）→ 执行器 → 位移 → 贴地 → 跳跃弹道 |
| `AiSet` | `decide_intent_system` → `enemy_declare_system` |
| `CombatSet` | 威胁检测 → 防御标记过期 → 菜单 → 声明 → 执行器 → 投射物到达/爆炸 → 目标获取 → 命中结算（防御/护甲/打断）→ 扣血 → 生命期清理 → 死亡销毁 |
| `PresentationSet` | 相机 → 纸片/阴影 → 日志 → 各面板 → 布局缩放 |

`CombatSet` 的完整链条见 [components.md](components.md) 第五节。

## 六、通信规范

**Message**（`#[derive(Message)]` + `app.add_message::<T>()`）用于跨系统的解耦广播；
**Event / EntityEvent + Observer** 只留给「即时、针对具体实体」的响应，两者不可混用。

消息一律「谁写 → 谁消费」成对，定义与消费系统同属一个领域目录，
注册在**消费方**插件的 `build` 里。总表：

| 消息 | 写 | 消费 |
| :--- | :--- | :--- |
| `MoveCommand` / `MoveToCommand` / `JumpCommand` | `input` / `interaction` | `movement` 的声明系统 |
| `FireCommand` / `MeleeCommand` | `input` / `menu` 派发 | `combat::skills` 的声明系统 |
| `RollCommand` / `ParryCommand` | `input` / `menu` 派发 | `combat::defense` 的声明系统 |
| `SelectSkill` / `CycleSkill` / `UseSelectedSkill` | `input` / `interaction` | `combat::skills::menu` |
| `PointerCommand` | `input` | `interaction::pointer_command_system` |
| `PlayerIntent` | `input`（键盘）/ `interaction`（左键） | `timeline::interrupt_system`（撤掉玩家那条还没到点的行动） |
| `UseFocus` / `UndoCommand` | `input` / `interaction` | `timeline` |
| `PauseRequest` | `input`（手动）/ `timeline` 的等输入系统 / `combat::reaction` | `timeline::process_pause_requests` → `apply_clock` |
| `InterruptEvent`（**EntityEvent**，住 `combat::formula`） | 命中系统 `apply_physical_hits_system` | `combat::formula::interrupt_observer`（判定与落地都在战斗域） |
| `ActionCancelled`（**EntityEvent**） | `timeline::undo_system`（销毁行动之前 trigger） | 花钱的领域：`combat::skills`（火球退 2 收 2、近战收 1） |
| `DecisionReady`（**EntityEvent**） | `timeline::recovery_system` | `combat::defense::recover_stamina_observer`（+1 精力） |
| `ActionBlocked` | 各声明系统（经 `timeline::FirstReady::first_ready`） | HUD 提示条 |
| `ProjectileArrived` | `skills::projectile_arrival_system` | `skills::explosion_system` |
| `DamageEvent` | 各伤害类型的命中系统（物理 / 爆炸） | `health::apply_damage_system`、战斗日志 |
| `DeathEvent` | `apply_damage_system` | 战斗日志（销毁由 `despawn_dead_system` 直接看 `Health`） |
| `PanCamera` / `ZoomCamera` / `ToggleHelp` / `PreviewReadout` | `input` / `interaction` | `presentation` |
| `ChunkLoadEvent` / `ChunkUnloadEvent` / `ChunkDirtyEvent` | `world` | `voxel_render`（+ `world` 自身的地形生成） |
| `ResetBattle` | `input::keyboard::restart_input_system`（`F5`） | `spawn::reset_battle_system` |

链式分工：**每种伤害类型一个组件 + 一个命中系统**（把"打到了谁"翻译成
`DamageEvent`）→ `apply_damage_system`（唯一扣血点）→ `DeathEvent`（首次归零）。
加一种伤害不需要动生命值、死亡、日志、撤销中的任何一处。

## 七、数据与表现分离（`world` / `voxel_render`）

- `world` 只回答「数据是什么」：区块（`Chunk` 持有 `Box<[VoxelType; 32³]>`）、
  噪声地形纯函数、体素读写 API。**不引用任何渲染类型**，因此
  `WorldPlugin` 能配 `MinimalPlugins` 直接单测（加载 → 生成 → 卸载）。
- `voxel_render` 只回答「怎么画」：脏区块 → 面剔除网格 → 按方块类型分组的网格实体。
  它只读世界数据，从不回写规则。
- 两域之间只有三条 Message，新增消费方（小地图、寻路缓存、存档）不需要改数据域。

**流式加载**：`ChunkLoader { radius }` 挂在玩家身上，`chunk_streaming_system` 每帧
汇总所有加载器覆盖的区块（切比雪夫范围、多加载器取并集），加载缺失的、
卸载离开范围的；地形由 `generate_terrain_system` 在收到 `ChunkLoadEvent` 后填充，
并标 `ChunkDirtyEvent`。默认 `radius = (0, 1, 0)` ⇒ **XZ 只加载 1×1 区块、Y 三层**，
区块边长 32 体素 = 64 世界单位，够覆盖当前相机视野。

**异步网格化**：`schedule_meshing_system` 把区块数据克隆给 `AsyncComputeTaskPool`，
任务句柄以 `MeshingTask` 挂在区块实体上；`apply_meshing_result_system` 每帧
`poll_once` 一次，完成后按方块类型各挂一个网格实体（`ChunkSurface`）。
区块被卸载 → 实体销毁 → 任务 drop → 后台计算自动取消，不需要额外取消协议。

v0.1 只做**面剔除**（被实心邻居挡住的面不生成）；贪婪网格化、纹理图集、AO
都是后续优化，且都不需要动数据域。

## 八、角色实体 = 组件的组合体

**没有任何模块叫「玩家」或「怪物」。** 一个实体只是多个领域提供的零件在同一实体上的组合，
组装代码集中在 `spawn/`：

```text
unit_scene        共用零件（Faction / Health / Collidable / HitRadius / AttackRange /
                  Velocity / DecisionSlot / Stamina / Cell + 2D 纸片 + 贴地阴影）
├── player_scene  + InputDriven（输入归属）+ MoveSpeed(5.0) + ChunkLoader
└── enemy_scene   + MoveSpeed(2.0) + EnemyBrain（#[require(Intent)]）

行动实体           声明系统 spawn_scene 出行动实体后 add_child 挂在行动者下（ChildOf）

setup_scene       方向光 → 相机 → 玩家 → 敌人 → 地表装饰（Startup）
reset_battle_system  清场 → 用同一组工厂重建（F5 的按键读取在 input，功能胶水不是领域）
```

玩家和敌人的差别只有两点：**驱动源**（`input` 写消息 vs `ai` 自己选意图）
与**特质零件**（区块加载器 / AI 大脑）。两者共用同一套 movement / combat / timeline 系统。

依赖方向单向：

```text
spawn ──▶ combat / movement / timeline / ai / world / presentation
input ──▶ movement / combat / timeline / interaction / presentation / spawn（只写消息）
interaction ──▶ movement / combat / timeline（点击解释成消息）
ai ──▶ movement / combat（只声明行动实体）
```

`input` 对 `spawn` 的依赖只有一条 `ResetBattle` 消息（`F5`），组装车间反过来
不认识键盘。行动实体的归属则走 **`ChildOf` 父子关系**：声明侧
`commands.spawn_scene(..).id()` + `commands.entity(actor).add_child(action)`，
读取侧（七个执行器、`undo_system`、`interrupt_observer`、`detect_threat_system`、
HUD 时间轴与行动行）用 `&ChildOf` 的 `parent()` 取行动者。

所以「加一种怪物」「换一套角色零件」永远不会波及战斗、移动、渲染的规则。

> Bevy 0.19 没有 `Bundle`：组装用 BSN 场景工厂表达（`bsn!` + `spawn_scene`）。
> 位置一律用 `Transform`，不另造 `Position` 组件（避免两份坐标真相）；
> 决策层坐标是独立的 `Cell`，只在单位**停下**时由 `move_entities_system` 维护。

## 九、功能不是领域

判断标准（新增同类功能时照此办理）：

- **有数据模型 / 有规则** → 进对应领域（新组件 + 新系统）；
- **只是把已有系统按顺序拼一次** → 留在调用方，别造领域。

`spawn/restart.rs` 是现成的例子：重置没有自己的数据模型，
它只是一段「清场 + 用同一套工厂重建」的胶水，因此 `ResetBattle` 消息与
消费系统（`reset_battle_system`）都留在同一个文件里——触发键 `F5` 的读取
则和其他按键一样住在 `input/keyboard.rs`，那里只翻译成消息。

清场查询只需要 `Faction` / `Projectile` / `Collidable`：没执行的行动是行动者的
**子实体**，人没了行动跟着没（`Children` 是 linked spawn），不必单独列进查询。

## 十、交互与表现

### 鼠标交互（`interaction`）

```text
光标 ─▶ cursor_ray（相机 → 世界射线） ─▶ pick_cell（沿地形高度场步进） ─▶ HoveredCell
                                                                    │
                          update_hover_highlight_system / update_preview_* ┘
```

拾取采样的是**高度场**而不是体素 DDA：当前世界没有悬垂，步进不需要读 `ChunkMap`、
不受区块加载影响，而且是纯函数（单测里手搓一条射线就能验）。等有洞穴再换 DDA，接口不变。

### HUD（`presentation/hud/`）

按屏幕位置拆成五块，每块一个文件 + 自己的更新系统：

| 位置 | 内容 | 文件 |
| :--- | :--- | :--- |
| 顶部 | 时间轴：按「执行时刻 − 前摇」长出的行动色块（蓝 = 玩家 / 红 = 敌人，草案半透明）+ 候场区 | `hud/timeline.rs` |
| 左下 / 右下 | 双方面板：头像 + HP / EN 条 + 状态行 + 当前行动 | `hud/panels.rs`、`hud/actions.rs` |
| 底部居中 | 技能栏：图标 + 消耗角标 + 悬停 tooltip | `hud/skills.rs` |
| 右下偏上 | 战斗日志：半透明、点标题折叠 | `hud/log_panel.rs` |
| 居中 / 左下 | 帮助面板（`F1`）与「无法操作 / 预演读数」提示条 | `hud/help.rs`、`hud/hint.rs` |

三条不变量：

1. **只读**：HUD 只把游戏状态映射成 `Node` / `Text`，绝不写回游戏数据；
2. **文案英文**：中文只出现在战斗日志正文（`presentation/log.rs`）；
3. **单一字体**：每段文本显式用 `HUD_FONT`（`assets/fonts/NotoSansSC-Regular.otf`，
   OFL-1.1）——Bevy 默认字体不含 CJK，日志正文会变豆腐块。

布局树与分辨率适配在 `hud/layout.rs`（`UiScale = 窗口高 / BASE_HEIGHT`）。
每个面板系统先算一份**纯数据快照**，与 `HudCache` 里的上一帧比对，相等就整帧不碰 UI 节点。
所有 HUD 节点都挂 `Name`，标记组件用 `register_type` 进反射表——
BRP 的 `world.query` 与按名截图靠它们定位实体；**加了新标记组件记得一起注册**。

## 十一、按键总表

| 键 / 操作 | 动作 |
| :--- | :--- |
| 方向键 | 走一格（方向**变化**时才发消息；WASD 让给技能热键） |
| 左键点地板 | 走到那一格（`MoveToCommand`，可跨多格） |
| 左键点单位 | 用当前选中的技能打那一格 |
| 右键 | 撤销未结算的玩家行动 |
| `1`~`4` | 直接放那一格的技能（选中 + 用一次） |
| `Tab` / `Shift+Tab` | 循环技能（只在当前负担得起的技能之间走） |
| `G` | 释放选中技能（`Attack` 按真实距离派发近战 / 火球） |
| `Q` / `W` / `E` / `R` | 技能热键，默认火球 / 近战 / 翻滚 / 招架（`HotkeyBinds`） |
| `C` | 跳跃（弹道约 0.6s，**不可取消**） |
| `Space` | 暂停 / 继续（只翻译成 `PauseRequest`，由暂停原因集合落地） |
| `Shift` + 决策键 | 用 1 点 Focus 把这一手的前摇归零 |
| `F1` | 帮助面板开合 |
| `F5` | 重置战斗 |
| 中键拖拽 / 滚轮 | 平移相机 / 缩放 |

方向键按**屏幕**算（上 = 远离相机），由 `input::GroundBasis` 按相机朝向换算到
世界 XZ 平面，再经 `movement::step_from_axis` 吸附成一格的正交步。

## 十二、与旧结构的对照

下面这些概念在代码里**已经不存在**，提到它们的地方都是历史（git 历史里可查）：

| 已删除 | 现在 |
| :--- | :--- |
| `Phase` / `RoundEnded` / `RESOLUTION_WINDOW` 阶段机 | `DecisionSlot` + 每动作的 `ActionTiming` |
| `ActionsCommitted` / `require_commit` / 等 `Enter` 确认 | 声明即生效；反悔走打断 / 撤销 |
| `Position` + `GridMath`（第二套网格坐标） | `Transform` + `Cell`（分工明确） |
| `Can*` 能力标记 | `DecisionSlot`（能不能决策）+ `SKILLS` 注册表 + `MenuSelection` |
| `AttackCooldown` 冷却计时器 | 后摇（`DecisionSlot::Recovery { until }`）就是冷却 |
| `AttackStats` 作为 ECS 组件 | 领域层的纯函数入参 |
| `Declared` / `Pending` / `Committed` 三态标记 | `DecisionSlot` 三态 + `ScheduledAction.execute_at` + `due()` / `pending()` |
| `timeline_gate_system` / `TimelineConfig` / `F2` 反应窗口 | `PauseReasons` + `PauseRequest` + 威胁检测 |
| `Arbitration` / `phase1_arbitrate` / `phase2_apply` / 三层裁决（破势） | 单一命中系统 + `InterruptEvent`（打断的是**还没到点**的行动） |
| `ModifyHealthEvent` / `AttackResolved` / `ActionCost` / `CancelCost` | `DamageEvent`（一段链路）/ 各领域自己的退款 Observer |
| `Busy` 后摇组件 / `Cancellable` 枚举 / `TogglePause` / `ManualPause` | `DecisionSlot::Recovery` / `Uncancellable` 标记 / `PauseRequest`（每帧断言） |
| `ScheduledAction.actor` 字段 | 行动是行动者的子实体（Bevy `ChildOf`） |
| `restart` / `scene` 等领域目录 | `spawn/` 组装车间 + 功能胶水 |
| `timeless/` workspace（代码 B） | 能力已迁入 `src/`，代码树已移除 |

## 十三、后续项

按优先级见 [../TODO.md](../TODO.md)。这里只列与架构直接相关的两条：

1. **动作数值外置**：各领域的 `*_TIMING` 与 `SKILLS` 的数值改成 `.ron`，应用层不再硬编码。
2. **单位贴地与体素碰撞**：`movement` 查询 `world` 的体素决定目标格是否可走
   （当前只贴地，不查可行走性）。
