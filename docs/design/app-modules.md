# 根目录 app 原型的领域化模块设计

> 依据 `req0.MD` 的「数据域 / 表现域分离 + 每域一个 Plugin + 按职责分文件」范式，
> 对根目录 `app` 原型（Bevy 0.19）做的一次结构重构。配套阅读：
> [架构原则与分层](architecture.md) · [Bevy 0.19 速查](../bevy/bevy-019.md)。

## 一、领域一览

一个领域 = 一个目录 = 一个 `Plugin`；领域内部按职责分文件
（`components` / `events` / `systems` / `resources`），`mod.rs` 只做门面
（`pub mod` + `pub use`），跨领域只经 Bevy `Message` 或公共组件类型通信。

| 领域 | 职责 | 插件 | 系统集 | 消息（谁写 → 谁消费） |
| :--- | :--- | :--- | :--- | :--- |
| `world` | 体素地图**数据**：区块、地形生成、体素读写（零渲染依赖） | `WorldPlugin` | `WorldSet` | `ChunkLoadEvent` / `ChunkUnloadEvent` / `ChunkDirtyEvent`（本域写 → `voxel_render` 消费） |
| `voxel_render` | 体素**表现**：异步网格化、材质、明暗 | `VoxelRenderPlugin` | `VoxelRenderSet` | 只消费上述三条区块消息 |
| `movement` | 速度与位移（位置直接用 Bevy `Transform`） | `MovementPlugin` | `MovementSet` | `MoveCommand`（`input` 写 → 本域消费） |
| `combat` | 生命 / 伤害 / 目标获取 / 攻击实体生命周期 / 技能生成 | `CombatPlugin` | `CombatSet` | `FireCommand`、`MeleeCommand`、`DamageEvent`、`ModifyHealthEvent`、`DeathEvent` |
| `timeline` | **We-Go 时间线**：规划阶段冻结虚拟时间等玩家提交，推进阶段按 `execute_at` 结算 | `TimelinePlugin` | `TimelineSet` | `ActionsCommitted`（`input` 写 → 本域消费）、`RoundEnded`（本域写 → movement / ai 消费） |
| `ai` | 敌人决策（规划阶段声明行动，不碰规则） | `AiPlugin` | `AiSet` | 无（读组件 + 生成自己的行动实体） |
| `input` | 玩家输入源：键盘 → 消息（只翻译） | `InputPlugin` | `InputSet` | `MoveCommand` / `FireCommand` / `MeleeCommand` / `ActionsCommitted` 的生产者 |
| `presentation` | 表现：相机（中键拖拽平移）/ 装饰 / 战斗日志 / HUD（将来还有动画 / 特效） | `PresentationPlugin` | `PreloadSet`（Startup）、`PresentationSet`（Update） | 消费 `DamageEvent` / `DeathEvent` 写日志；消费 `PanCamera` 平移相机 |
| `spawn` | **组装车间**：把各域零件拼成角色实体；含开局组装与「重建」功能 | `SpawnPlugin` | `AssemblySet`（Startup）、`SpawnSet`（Update） | `ResetBattle`（本域内闭环：功能自带触发键与消费系统） |

执行顺序只在 `GamePlugin` 里声明一次（`configure_pipeline`，测试也复用同一入口），
领域内部顺序由各自插件维护：

```text
Startup:  PreloadSet ─▶ AssemblySet
Update:   SpawnSet ─▶ InputSet ─▶ TimelineSet ─▶ AiSet ─▶ MovementSet ─▶ CombatSet
          ─▶ VoxelRenderSet ─▶ PresentationSet
WorldSet ────────────────────────▶（必须早于 VoxelRenderSet）
```

## 二、We-Go 回合循环（暂停等待用户输入）

```text
              Enter（玩家提交）
Planning ─────────────────▶ Resolving ──（1s 窗口走完，广播 RoundEnded）──▶ Planning
虚拟时间：暂停              流动                                            暂停
```

- **Planning**：`Time<Virtual>` 冻结，游戏真的停下来等玩家。玩家用 `WASD`
  声明移动、`Space` 声明射击、`E` 声明近战，`Enter` 提交；敌人也在同一阶段
  声明自己的行动（[`crate::ai::enemy_declare_system`]）。
- **Resolving**：提交把本轮所有 `Declared` 草案一并变成 `Pending`，并按
  「提交时刻 + 前摇」定下 `execute_at`；到点后调度器标记 `Committed`，
  载荷领域的执行器落地（移动设速度、技能生成攻击实体）。
- **窗口结束**：没执行完的行动作废，`RoundEnded` 让单位停下、AI 走一格冷却，
  虚拟时间重新冻结。

行动的三个状态标记（`Declared` / `Pending` / `Committed`）与调度数据
`ScheduledAction` 全在 `timeline/components.rs`；**调度器不感知载荷**——
`MoveAction` 在 [`crate::movement::actions`]，`ShootAction` / `MeleeAction` 在
[`crate::combat::skills::actions`]，新增动作（冲刺、翻滚、火球…）只加载荷与执行器。

暂停只用 `Time<Virtual>` 实现：Bevy 每帧把虚拟时间拷进通用 `Time`，因此位移、
计时器、攻击存活期全部自动停表，没有任何 `if paused` 分支（AGENTS.md：暂停用
`Time<Virtual>`，不要手写阶段门控）。

按键：`WASD`/方向键 移动 · `Space` 射击 · `E` 近战 · `Enter` 提交 · `R` 重置 ·
按住鼠标中键拖拽平移相机。

HUD（`presentation/hud.rs`）显示：阶段与剩余窗口时间、轮次、双方血量 / 坐标 /
本轮声明 / 距离 / 敌人冷却、按键提示与战斗日志尾部。HUD 文本一律 ASCII——
Bevy 默认字体不含 CJK，中文界面需要自带字体资产（见 TODO 的表现层待办）。
相机平移同样遵守「输入只翻译」：`input/pointer.rs` 把中键拖拽翻译成 `PanCamera`
消息，`presentation/camera.rs` 的 `CameraRig` 消费它（注视点限制在场地范围内）。

## 三、角色实体 = 组件的组合体（最重要的约定）

**没有任何模块叫「玩家」或「怪物」。** 一个敌人实体只是多个领域提供的组件
在同一实体上的组合：

| 组件 | 提供方 |
| :--- | :--- |
| `Health` | `combat::health` |
| `PhysicalDamage` / `Armor` / `HitRadius` | `combat::attributes` |
| `Faction` / `Collidable` | `combat`（目标过滤的公共词汇） |
| `Velocity` / `MoveSpeed` | `movement` |
| `EnemyBrain` / `AttackCooldown` | `ai` |
| `ChunkLoader`（仅玩家） | `world` |
| 模型（`WorldAssetRoot`）/ 相机 / 装饰 / 日志 | `presentation` |

玩家和敌人的差别只有两点：**驱动源**（`input` 写 `MoveCommand` vs `ai` 直接写
`Velocity`）与**特质零件**（区块加载器 / 攻击冷却）；两者共用同一套 `movement` /
`combat` 系统。

组装代码集中在一个 **`spawn` 组装车间**（`unit.rs` / `player.rs` / `enemy.rs`），
它依赖所有领域，但**没有任何领域依赖它**：

```text
spawn ──▶ combat / movement / ai / world / presentation
input ──▶ movement / combat / timeline（只写它们的消息）
ai    ──▶ movement / combat（只声明行动实体）
```

所以「加一种怪物」「换一套角色零件」永远不会波及战斗、移动、渲染的规则。

> Bevy 0.19 没有 `Bundle`，组装用 BSN 场景工厂表达（`bsn!` + `spawn_scene`）：
> `spawn/unit.rs` 给出共用零件，`player.rs` / `enemy.rs` 追加各自零件。
> 位置一律用 Bevy 的 `Transform`，不另造 `Position` 组件（避免两份坐标真相）。

## 四、功能不是领域（`restart` 的落点）

「战斗重置」没有自己的数据模型，它只是**一段功能胶水**：清掉单位与攻击实体，
再用同一套工厂组装一次。因此它既不属于 ECS 数据域、也不属于表现域，而是作为
功能留在组装车间里（`spawn/restart.rs`：消息 `ResetBattle` + R 键翻译 +
消费系统同文件）。

判断标准（新增同类功能时照此办理）：

- **有数据模型 / 有规则** → 进对应领域（新组件 + 新系统）；
- **只是把已有系统按顺序拼一次** → 留在调用方（`spawn` 或游戏流程），别造领域。

## 五、目录结构

```
src/
├── main.rs                     # 只加引擎插件 + GamePlugin
├── lib.rs                      # GamePlugin / configure_pipeline
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
├── movement/{components,events,systems,plugin}.rs
│   └── actions.rs              #   MoveAction 载荷 + 工厂 + 声明 / 执行器
├── combat/
│   ├── plugin.rs               #   CombatPlugin（战斗流水线）
│   ├── components.rs           #   Faction / Collidable
│   ├── attributes/components.rs#   PhysicalDamage / Armor / HitRadius
│   ├── health/{components,events,systems}.rs
│   ├── formula/{types,events,systems}.rs
│   ├── targeting/{components,detection,melee}.rs
│   ├── lifecycle/{components,systems}.rs
│   └── skills/{events,arrow,melee}.rs + skills/actions.rs
│                               #   ShootAction / MeleeAction 载荷 + 工厂 + 声明 / 执行器
├── timeline/                   # We-Go 时间线（规划 / 推进）
│   ├── resources.rs            #   Timeline / Phase / 窗口时长
│   ├── components.rs           #   ScheduledAction + Declared / Pending / Committed
│   ├── events.rs               #   ActionsCommitted（提交）、RoundEnded（收尾）
│   └── systems.rs              #   暂停门控 / 提交 / 调度 / 窗口收尾
├── ai/{components,systems,plugin}.rs
├── input/{keyboard,plugin}.rs   # 键盘 → 消息（只翻译）
├── presentation/{components,camera,decoration,log,preload,plugin}.rs
└── spawn/                       # 组装车间
    ├── unit.rs                  #   玩家 / 敌人共用的单位零件
    ├── player.rs                #   单位零件 + 输入驱动 + ChunkLoader
    ├── enemy.rs                 #   单位零件 + AI 驱动 + 攻击冷却
    ├── assembly.rs              #   开局组装（灯光 / 相机 / 单位 / 装饰）
    ├── restart.rs               #   功能：战斗重置（消息 + 触发键 + 系统）
    └── plugin.rs                #   SpawnPlugin
```

## 六、关键设计点

### 1. 数据与表现分离（`world` / `voxel_render`）

- `world` 只回答「数据是什么」：区块（`Chunk` 持有 `Box<[VoxelType; 32³]>`）、
  地形生成（噪声纯函数）、体素读写 API。**不引用任何渲染类型**，因此可以脱离渲染
  环境单测（`world/plugin.rs` 的测试就是这条约束的可执行证明：`MinimalPlugins` +
  `WorldPlugin` 直接跑通「加载 → 生成 → 卸载」）。
- `voxel_render` 只回答「怎么画」：脏区块 → 面剔除网格 → 按方块类型分组的网格实体。
  它**只读**世界数据，从不回写规则。
- 两个领域之间只有三条 Message（`ChunkLoad` / `ChunkUnload` / `ChunkDirty`），
  新增消费方（小地图、寻路缓存、存档）不需要改数据域。

### 2. 区块流式加载

`ChunkLoader { radius: IVec3 }` 挂在玩家身上即可：`chunk_streaming_system` 每帧
汇总所有加载器覆盖的区块（切比雪夫范围，多加载器取并集），加载缺失区块并发
`ChunkLoadEvent`，卸载离开范围的区块并发 `ChunkUnloadEvent`；地形由
`generate_terrain_system` 在收到加载消息后填充，并标记 `ChunkDirtyEvent`。

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

`input/` 是唯一的输入翻译层：键盘只把按键翻译成消息（`MoveCommand` /
`FireCommand` / `MeleeCommand`），由对应领域的单一职责系统消费落地；消息定义与
消费系统同属一个领域目录，注册在各领域插件的 `build` 里（`add_message::<T>()`），
生产者只引用消息类型、不引用消费系统。功能自带的触发键（R 重置）跟着功能走，
见 `spawn/restart.rs`。

### 5. 战斗流水线（`CombatSet` 内部链）

```text
skills（规划阶段：消费技能指令 → 声明技能行动）
  ─▶ skills 执行器（到点：从行动者位置生成箭矢 / 横扫，销毁行动实体）
  ─▶ targeting（挂 CollisionTarget：射弹碰撞 / 近战扇形）
  ─▶ formula（算伤害 → DamageEvent）
  ─▶ lifecycle（命中计数、结束时归零速度）
  ─▶ health（DamageEvent → ModifyHealthEvent → 扣血 → DeathEvent → 销毁实体）
  ─▶ lifecycle（清理结束的射弹、到期销毁一次性攻击）
```

同一套骨架也是移动领域的写法：
`declare_move_system`（规划阶段声明）→ `move_action_executor_system`（到点设速度）
→ `move_entities_system`（按速度位移）→ `stop_on_round_end_system`（本轮结束停下）。

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
| `control/*`、`movement/input.rs`、`combat/skills/input.rs` | `input/keyboard.rs` + `movement/events.rs` |
| `combat/movement.rs`（速度位移） | `movement/systems.rs::move_entities_system` |
| `despawn/*` | `combat/health/systems.rs::despawn_dead_system`（消息与消费同域） |
| `battlelog/*` | `presentation/log.rs` |
| `camera.rs` / `decoration.rs` | `presentation/{camera,decoration}.rs` |
| `character.rs` / `scene/unit.rs`（角色工厂） | `spawn/{unit,player,enemy}.rs`（组装车间） |
| `restart/*`（曾是一个领域） | `spawn/restart.rs`（功能，不是领域） |
| —（新增） | `timeline/`：规划 / 推进两阶段 + 行动实体调度（We-Go 的「暂停等输入」） |
| `movement` / `combat` 里即时生效的系统 | 改为「声明行动 → 到点执行」：`movement/actions.rs`、`combat/skills/actions.rs` |
| `lib.rs` 的 `preload` / `setup` / `add_combat` | `presentation/preload.rs`、`spawn/assembly.rs` 与各领域 `plugin.rs` |
| `map.rs`（地面 / 网格平面） | 由 `world` + `voxel_render` 的体素地形取代 |

## 八、后续项（按优先级）

1. **单位贴地与体素碰撞**：`movement` 查询 `world` 的体素判断目标格是否可走
   （当前单位只在生成时贴地，不跟随地形爬坡）。
2. **反应窗口**：推进阶段的前摇窗口内允许翻滚取消 / 招架（timeline 已按
   `execute_at` 调度，插入反应只需再加一条载荷与执行器）。
3. **贪婪网格化**：把同材质共面合并成矩形，替代逐面四边形。
4. **纹理图集**：`materials/assets.rs` 换成图集 + UV，网格化代码不动。
5. **AO**：`lighting/systems.rs` 目前只做面朝向明暗，可替换为按顶点的邻域遮挡。
6. **区块持久化与钉住**：`ChunkPinned` + 存档，只保存被修改过的区块。
7. **方块交互**：放置 / 破坏走 `world::storage::set_voxel`，自动触发重建网格。
8. **表现层补零件**：血条 / 动画 / 特效（新零件进 `presentation`，组装语句进 `spawn`）。
