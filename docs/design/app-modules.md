# 根目录 app 原型的领域化模块设计

> **范围**：仓库根目录 `src/`（package `app`，Bevy 0.19）——**代码 A，当前主线**。
> 本文与代码同步维护。
>
> 配套阅读：[无回合时间线](timeline-turnless.md)（权威设计） ·
> [ECS 战斗组件](ecs-combat-components.md) · [Bevy 0.19 速查](../bevy/bevy-019.md)。
>
> 本文替代 2026-09 之前的同名文档：那一版写的是已删除的 We-Go 版本
> （`Phase` / `RoundEnded` / `Position`+`GridMath` / `Can*` 能力标记）。

## 一、领域一览

一个领域 = 一个目录 = 一个 `Plugin`；领域内部按职责分文件
（`components` / `events` / `systems` / `resources`），`mod.rs` 只做门面
（`pub mod` + `pub use`），跨领域只经 Bevy `Message` 或公共组件类型通信。

| 领域 | 职责 | 插件 | 系统集 |
| :--- | :--- | :--- | :--- |
| `world` | 体素地图**数据**：区块、地形生成、体素读写（零渲染依赖） | `WorldPlugin` | `WorldSet` |
| `voxel_render` | 体素**表现**：异步网格化、材质、明暗 | `VoxelRenderPlugin` | `VoxelRenderSet` |
| `movement` | 格子决策 + 速度位移 + 移动/跳跃/翻滚行动 + 投射物飞行 | `MovementPlugin` | `MovementSet` |
| `combat` | 生命 / 伤害 / 目标获取 / 攻击生命周期 / 技能 / 精力 / 防御 / 两阶段结算 | `CombatPlugin` | `CombatSet` |
| `timeline` | **无回合**调度：谁能决策（`Ready`）、行动何时到点、后摇何时结束 | `TimelinePlugin` | `TimelineSet` |
| `ai` | 敌人决策（选意图 + 声明行动，不碰规则） | `AiPlugin` | `AiSet` |
| `input` | 玩家输入源：键盘 / 鼠标 → 消息（**只翻译**） | `InputPlugin` | `InputSet` |
| `interaction` | 鼠标交互：射线拾取悬停格 → 高亮 / 预演指示器；点击 → 各领域的消息 | `InteractionPlugin` | `InteractionSet` |
| `presentation` | 表现：相机 / 单位纸片与贴地阴影 / 装饰 / 战斗日志 / HUD（时间轴 · 面板 · 技能栏 · 日志 · 帮助，只读） | `PresentationPlugin` | `PreloadSet`(Startup)、`PresentationSet`(Update) |
| `spawn` | **组装车间**：把各域零件拼成角色实体；含开局组装与「重建」 | `SpawnPlugin` | `AssemblySet`(Startup)、`SpawnSet`(Update) |

消息全部「谁写 → 谁消费」成对，清单见
[ECS 战斗组件 · 第五节](ecs-combat-components.md#五消息清单谁写--谁消费)。

执行顺序只在 `lib.rs::configure_pipeline` 里声明一次（测试复用同一入口，
因此测试跑的就是真实流水线）：

```text
Startup:  PreloadSet ─▶ AssemblySet
Update:   SpawnSet ─▶ InputSet ─▶ TimelineSet ─▶ AiSet ─▶ MovementSet ─▶ CombatSet
          ─▶ VoxelRenderSet ─▶ PresentationSet
WorldSet ────────────────────────▶（必须早于 VoxelRenderSet：数据先于表现）
```

## 二、无回合循环（唯一的暂停点是「等玩家决定」）

**没有回合、没有阶段、没有全局状态机。** 节奏由两件事决定：

1. **`Ready`**：单位「现在可以决策」。这是无回合模型里**唯一的「轮到谁」判据**。
2. **`ActionTiming { windup, recovery }`**：每个动作自带的前摇 + 后摇。

```text
       玩家 Ready ⟹ 冻结 Time<Virtual>（世界真的停下来等）
输入 ─▶ 声明动作（移除 Ready）⟹ 虚拟时间恢复流动
       ─▶ 前摇到点 → 执行器落地 → 后摇（BusyRecovery）
       ─▶ 后摇结束 ⟹ 恢复 Ready（+1 精力）⟹ 又轮到它
```

敌人不等玩家：它一有 `Ready` 就自己决策（`ai::decide_intent_system` →
`enemy_declare_system` 写与玩家**同一条**消息）。玩家与 AI 因此共用同一套
声明 → 调度 → 执行链，防御与技能都只有一份实现。

冻结的判据（`timeline_gate_system`，整个游戏唯一按游戏状态写 `Time<Virtual>` 的地方）：

```text
冻结 ⟺ 场上存在玩家 且 没有单位在空中 且（玩家 Ready 或 反应窗口判定有威胁）
```

- **为什么「空中不冻结」**：跳跃是不可中断的弹道。若玩家落地前恢复 `Ready`
  就停表，单位会僵在半空。
- **为什么各领域没有 `if paused`**：Bevy 每帧把虚拟时间拷进通用 `Time`，
  所以位移、投射物、`Lifetime`、后摇计时**自动**停表（AGENTS.md：
  暂停用 `Time<Virtual>`，不要手写阶段门控）。
- **反悔不用"确认"**：声明即生效（`commit_bridge_system` 当帧升 `Pending`），
  改主意走打断 / 撤销——右键，或者直接按下一个新意图；能撤到什么时候由行动自己说
  （`CancelCost` / `Uncancellable`）。`F2` 只调**反应窗口**的松紧
  （`Loose` / `Strict` / `Off`），不是提交开关。

按键：方向键走一格 · 左键点地板走 / 点单位用选中技能 · 右键撤销 · `1`~`4` 直接放技能 ·
`Tab`/`Shift+Tab` 循环 · `G` 释放选中技能 · `Q`/`W`/`E`/`R` 技能热键 · `C` 跳跃 ·
`Space` 暂停 · `F1` 帮助 · `F2` 循环反应窗口 · `F5` 重置 · 中键拖拽平移相机 · 滚轮缩放。

移动方向按**屏幕**算（上 = 远离相机），由 `input` 的 `GroundBasis` 按相机朝向
换算到世界 XZ 平面，再经 `movement::step_from_axis` **吸附成一格的正交步**。
**决策按格、结算按真实距离**的分工见
[ECS 战斗组件 · 第二节](ecs-combat-components.md#二坐标两套坐标各管一段)。

跳跃（`C`）也是移动领域的行动载荷（`JumpAction` + `Jumping` 弹道）：
到点后给行动者一个向上初速度，落回起跳高度即结束；虚拟时间冻结时弹道一起冻住。

单位外观走**伪 3D**（`presentation/unit_sprite.rs`）：玩家 / 敌人是 2D 精灵
（billboard，每帧绕 Y 轴对准相机），高度用**正下方地表上的黑色阴影**表示——阴影贴在
单位所在 XZ 的地表高度上，离地越远越小，因此「脚底到阴影的距离」就是可以直接读出的
高度差。精灵与阴影都是单位实体的子节点，所以单位根节点保持「脚底 + 无旋转 + 无缩放」；
树木 / 石头等装饰仍是 3D glTF 模型。

HUD 按**屏幕位置**拆成五块（`presentation/hud/`，每块一个文件 + 自己的更新系统）：

| 位置 | 内容 | 文件 |
| :--- | :--- | :--- |
| 顶部 | 时间轴：`execute_at` 排序的行动色块（蓝 = 玩家 / 红 = 敌人，草案半透明） | `hud/timeline.rs` |
| 左下 / 右下 | 双方面板：头像 + HP / EN 条 + 状态行 + 当前行动 | `hud/panels.rs`、`hud/actions.rs` |
| 底部居中 | 技能栏：图标按钮 + 消耗角标 + 悬停 tooltip | `hud/skills.rs` |
| 右下偏上 | 战斗日志：半透明、点标题折叠 | `hud/log_panel.rs` |
| 居中 | 帮助面板（`F1`，常驻按键提示只在这里） | `hud/help.rs` |

布局树与分辨率适配在 `hud/layout.rs`（`UiScale` = 窗口高 / `BASE_HEIGHT`）。
每个面板系统都先算一份**纯数据快照**，与 `HudCache` 里的上一帧比对，相等就整帧不碰
UI 节点（日志用 `Res::is_changed()` + 折叠状态）；WeGo 冻结时 HUD 长期不产生写入。
**HUD 是只读的**：HP / EN 条是进度条而不是可拖动的控件；**文案一律英文**——
中文只出现在战斗日志正文，Bevy 默认字体不含 CJK，所以要自带字体资产。
相机平移同样遵守「输入只翻译」：
`input/pointer.rs` 把中键拖拽翻译成 `PanCamera` 消息，
`presentation/camera.rs` 的 `CameraRig` 消费它（注视点限制在场地范围内）。

## 三、角色实体 = 组件的组合体（最重要的约定）

**没有任何模块叫「玩家」或「怪物」。** 一个实体只是多个领域提供的组件
在同一实体上的组合：

| 组件 | 提供方 |
| :--- | :--- |
| `Health` | `combat::health` |
| `Faction` / `Collidable` / `HitRadius` / `AttackRange` | `combat`（目标过滤与武器属性的公共词汇） |
| `PhysicalDamage` / `Armor` / `AttackFrame` / `Impact` | `combat::attributes` |
| `Stamina` | `combat::defense` |
| `Cell` / `Velocity` / `MoveSpeed` | `movement` |
| `Ready` | `timeline` |
| `EnemyBrain` / `Intent` | `ai` |
| `ChunkLoader`（仅玩家） | `world` |
| 2D 精灵与贴地阴影 / 相机 / 装饰 / 日志 | `presentation` |

玩家和敌人的差别只有两点：**驱动源**（`input` 写消息 vs `ai` 自己选意图）
与**特质零件**（区块加载器）；两者共用同一套 `movement` / `combat` / `timeline` 系统。

组装代码集中在一个 **`spawn` 组装车间**（`unit.rs` / `player.rs` / `enemy.rs`），
它依赖所有领域，但**没有任何领域依赖它**：

```text
spawn ──▶ combat / movement / timeline / ai / world / presentation
input ──▶ movement / combat / timeline / presentation（只写它们的消息）
ai    ──▶ movement / combat / defense（只声明行动实体）
```

所以「加一种怪物」「换一套角色零件」永远不会波及战斗、移动、渲染的规则。

> Bevy 0.19 没有 `Bundle`，组装用 BSN 场景工厂表达（`bsn!` + `spawn_scene`）：
> `spawn/unit.rs` 给出共用零件，`player.rs` / `enemy.rs` 追加各自零件。
> 位置一律用 Bevy 的 `Transform`，不另造 `Position` 组件（避免两份坐标真相）；
> 决策层坐标是独立的 `Cell`，由 `move_entities_system` 在**停下**时维护。

## 四、功能不是领域（`restart` 的落点）

「战斗重置」没有自己的数据模型，它只是**一段功能胶水**：清掉单位与攻击实体，
再用同一套工厂组装一次。因此它既不属于 ECS 数据域、也不属于表现域，而是作为
功能留在组装车间里（`spawn/restart.rs`：消息 `ResetBattle` + `R` 键翻译 +
消费系统同文件）。

判断标准（新增同类功能时照此办理）：

- **有数据模型 / 有规则** → 进对应领域（新组件 + 新系统）；
- **只是把已有系统按顺序拼一次** → 留在调用方（`spawn` 或游戏流程），别造领域。

## 五、目录结构

```text
src/
├── main.rs                     # 只加引擎插件 + GamePlugin
├── lib.rs                      # GamePlugin / configure_pipeline / 整机集成测试
├── world/                      # 体素地图数据（纯数据，零渲染依赖）
│   ├── plugin.rs               #   WorldPlugin
│   ├── voxel/{components,types}.rs
│   ├── chunk/{components,events,systems}.rs
│   ├── terrain/{resources,systems}.rs
│   └── storage/{resources,systems}.rs
├── voxel_render/               # 体素表现（渲染）
│   ├── plugin.rs               #   VoxelRenderPlugin
│   ├── meshing/{components,resources,systems,utils}.rs
│   ├── materials/{assets,resources}.rs
│   └── lighting/systems.rs
├── movement/
│   ├── cell.rs                 #   Cell / MoveGoal（决策层坐标）
│   ├── components.rs           #   Velocity / MoveSpeed
│   ├── events.rs               #   MoveCommand / MoveToCommand / JumpCommand
│   ├── actions.rs              #   MoveAction / JumpAction / RollAction + 工厂 + 声明/执行器
│   ├── systems.rs              #   move_entities_system / follow_terrain_system + DodgingOnArrival
│   └── plugin.rs
├── combat/
│   ├── components.rs           #   Faction / Collidable
│   ├── attributes/components.rs#   PhysicalDamage / Armor / HitRadius / AttackRange / AttackFrame / Impact
│   ├── health/{components,events,systems}.rs
│   ├── targeting/{components,detection,melee}.rs
│   ├── lifecycle/{components,systems}.rs
│   ├── formula/                #   domain(零 Bevy 裁决) / events / resolution(两阶段 + Arbitration)
│   ├── defense/                #   stamina / components / actions / systems / events
│   ├── skills/                 #   registry / menu / melee / arrow / fireball / explosion / actions / events
│   └── plugin.rs               #   CombatPlugin（战斗流水线）
├── timeline/
│   ├── timing.rs               #   ActionTiming + 常量表 + CELL_SIZE
│   ├── components.rs           #   ScheduledAction + Declared / Pending / Committed / Ready / BusyRecovery
│   ├── resources.rs            #   Timeline / TimelineConfig
│   ├── events.rs               #   TogglePause / CycleReactionWindow / ActionBlocked / UndoCommand / ActionCancelled
│   ├── systems.rs              #   门控 / 暂停 / 打断 / 提交桥 / 撤销 / 调度 / 后摇 + begin_action / end_action
│   └── plugin.rs
├── ai/{components,systems,plugin}.rs
├── input/{keyboard,pointer,plugin}.rs   # 键盘 / 鼠标 → 消息（只翻译）
├── interaction/                # 鼠标交互：拾取 / 高亮 / 点击 → 消息 / 预演指示器
│   ├── components.rs           #   HoveredCell（资源）/ HoverHighlight / HoverTint / AoePreview / ConePreview
│   ├── events.rs               #   PointerCommand
│   ├── raycast.rs              #   cursor_ray + pick_cell（沿高度场步进的纯函数）
│   ├── systems.rs              #   悬停 / 高亮 / 预演 / 读数 / 点击解释
│   └── plugin.rs
├── presentation/{components,camera,unit_sprite,decoration,log,preload,plugin}.rs
├── presentation/hud/{mod,layout,panels,actions,skills,timeline,log_panel,hint,help}.rs
└── spawn/                       # 组装车间
    ├── unit.rs                  #   玩家 / 敌人共用的单位零件
    ├── player.rs                #   单位零件 + 输入驱动 + ChunkLoader
    ├── enemy.rs                 #   单位零件 + AI 驱动
    ├── assembly.rs              #   开局组装（灯光 / 相机 / 单位 / 装饰）
    ├── restart.rs               #   功能：战斗重置（消息 + 触发键 + 系统）
    └── plugin.rs                #   SpawnPlugin
```

## 六、关键设计点

### 1. 数据与表现分离（`world` / `voxel_render`）

- `world` 只回答「数据是什么」：区块（`Chunk` 持有 `Box<[VoxelType; 32³]>`）、
  地形生成（噪声纯函数）、体素读写 API。**不引用任何渲染类型**，因此可以脱离
  渲染环境单测（`world/plugin.rs` 的测试就是这条约束的可执行证明：
  `MinimalPlugins` + `WorldPlugin` 直接跑通「加载 → 生成 → 卸载」）。
- `voxel_render` 只回答「怎么画」：脏区块 → 面剔除网格 → 按方块类型分组的网格实体。
  它**只读**世界数据，从不回写规则。
- 两个领域之间只有三条 Message（`ChunkLoad` / `ChunkUnload` / `ChunkDirty`），
  新增消费方（小地图、寻路缓存、存档）不需要改数据域。

### 2. 区块流式加载

`ChunkLoader { radius: IVec3 }` 挂在玩家身上即可：`chunk_streaming_system` 每帧
汇总所有加载器覆盖的区块（切比雪夫范围，多加载器取并集），加载缺失区块并发
`ChunkLoadEvent`，卸载离开范围的区块并发 `ChunkUnloadEvent`；地形由
`generate_terrain_system` 在收到加载消息后填充，并标记 `ChunkDirtyEvent`。
玩家半径是 3×3 个区块——相机是固定机位，只加载脚下那一块会在走动时把地面「抽走」。

已修改区块可挂 `ChunkPinned` 免于自动卸载（供后续「玩家建造 / 战斗破坏地形」使用）。

### 3. 异步网格化

网格化是 CPU 密集任务：`schedule_meshing_system` 把区块数据克隆给
`AsyncComputeTaskPool`，任务句柄以 `MeshingTask` 挂在区块实体上；
`apply_meshing_result_system` 每帧轮询一次（`poll_once`），完成后按方块类型各挂一个
网格实体（`ChunkSurface`）。区块被卸载时实体销毁 → 任务被 drop → 后台计算自动取消，
不需要额外的取消协议。

v0.1 只做**面剔除**（`MeshingConfig::cull_hidden_faces`，被实心邻居挡住的面不生成）；
贪婪网格化、纹理图集、AO 都留作后续优化，且都不需要动数据域。

### 4. 输入只翻译、不执行

`input/` 是唯一的输入翻译层：键盘只把按键翻译成消息，鼠标只把拖拽翻译成
`PanCamera`，由对应领域的单一职责系统消费落地。消息定义与消费系统同属一个领域
目录，注册在各消费方领域插件的 `build` 里（`add_message::<T>()`），
生产者只引用消息类型、不引用消费系统。

**技能菜单也遵守这条**：`SelectSkill` / `CycleSkill` 只改 `MenuSelection`
（一个资源），`UseSelectedSkill` 由派发系统按当前选择再写
`MeleeCommand` / `FireCommand` / `RollCommand`——菜单**不生成行动实体、不扣精力**，
扣费只在各领域的声明系统里发生。功能自带的触发键（`F5` 重置）跟着功能走，
见 `spawn/restart.rs`。

### 5. 战斗流水线（`CombatSet` 内部链）

```text
防御标记过期（本帧到期的无敌帧不该再生效）
  ─▶ 技能菜单（选择 / 循环 / 按选择派发）
  ─▶ 声明（火球 / 近战 / 翻滚 / 招架：检查 Ready 与精力，生成行动实体）
  ─▶ 执行器（到点：设速度、挂防御标记、生成攻击实体，收尾走 end_action）
  ─▶ 投射物到达 + 爆炸（本帧到达本帧结算，按真实距离取半径内敌对单位）
  ─▶ targeting（挂 CollisionTarget：射弹碰撞 / 近战扇形）
  ─▶ 两阶段结算（阶段 1 只读裁决 → 阶段 2 统一落地，经 Arbitration 资源交接）
  ─▶ health（DamageEvent → ModifyHealthEvent → 扣血 → DeathEvent → 销毁实体）
  ─▶ lifecycle（清理结束的射弹、到期销毁一次性攻击）
```

同一套骨架也是移动领域的写法：
`declare_move_system`（声明）→ `move_action_executor_system`（到点设速度 + 挂
`MoveGoal`）→ `move_entities_system`（位移 + 到格中心吸附 + 写 `Cell`）。

「打到了谁」（targeting）与「打多少血」（formula）靠临时标记 `CollisionTarget`
解耦；生命值只认自己的 `ModifyHealthEvent` 入口，治疗 / 中毒 / 再生都能复用同一条路。

## 七、与旧结构的对照

| 旧落点（重构前） | 新落点 |
| :--- | :--- |
| `events.rs`（全局伤害 / 死亡消息） | `combat/formula/events.rs`、`combat/health/events.rs` |
| `combat/health.rs` | `combat/health/{components,events,systems}.rs` |
| `combat/damage/*` | `combat/formula/*` |
| `combat/components.rs`（混合） | `combat/components.rs` + `attributes/` + `targeting/` + `lifecycle/` |
| `attacks/*`（箭矢 / 近战 / 输入） | `combat/skills/*`（输入另见下行） |
| `control/*`、`movement/input.rs` | `input/keyboard.rs` + `movement/events.rs` |
| `combat/movement.rs`（速度位移） | `movement/systems.rs::move_entities_system` |
| `despawn/*` | `combat/health/systems.rs::despawn_dead_system`（消息与消费同域） |
| `battlelog/*` | `presentation/log.rs` |
| `camera.rs` / `decoration.rs` | `presentation/{camera,decoration}.rs` |
| `character.rs` / `scene/unit.rs`（角色工厂） | `spawn/{unit,player,enemy}.rs`（组装车间） |
| `restart/*`（曾是一个领域） | `spawn/restart.rs`（功能，不是领域） |
| `timeline/`：规划 + 推进两阶段（We-Go） | `timeline/`：**无回合**调度（`Ready` + 每动作前后摇） |
| `movement` / `combat` 里即时生效的系统 | 「声明行动 → 到点执行」：`movement/actions.rs`、`combat/skills/` |
| `lib.rs` 的 `preload` / `setup` / `add_combat` | `presentation/preload.rs`、`spawn/assembly.rs` 与各领域 `plugin.rs` |
| `map.rs`（地面 / 网格平面） | 由 `world` + `voxel_render` 的体素地形取代 |

## 八、后续项（按优先级）

1. **单位贴地与体素碰撞**：`movement` 查询 `world` 的体素判断目标格是否可走
   （当前单位只在生成时贴地，不跟随地形爬坡；起点因此在格角上）。
2. **动作数值外置**：`timeline::timing` 与 `SKILLS` 的数值改成 `.ron`
   （见 [timeline-turnless](timeline-turnless.md) 6.6）。
3. **箭矢接回输入**：`ShootAction` / `arrow_scene` 已实现但未注册
   （避免与火球抢同一条 `FireCommand`），计划作为「单体狙击」技能。
4. **贪婪网格化**：把同材质共面合并成矩形，替代逐面四边形。
5. **纹理图集**：`materials/assets.rs` 换成图集 + UV，网格化代码不动。
6. **AO**：`lighting/systems.rs` 目前只做面朝向明暗，可替换为按顶点的邻域遮挡。
7. **区块持久化与钉住**：`ChunkPinned` + 存档，只保存被修改过的区块。
8. **方块交互**：放置 / 破坏走 `world::storage::set_voxel`，自动触发重建网格。
9. **表现层补零件**：血条 / 动画 / 特效（新零件进 `presentation`，组装语句进 `spawn`）。
