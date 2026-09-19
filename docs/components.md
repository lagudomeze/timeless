# 组件 → 系统对照（底层零件到场景组合）

> ⚠️ **本篇正在被拆分取代**：域与契约 → [domain.md](domain.md)；时间线与行动 → [timeline.md](timeline.md)；战斗与对抗 → [combat.md](combat.md)；技能 → [skills.md](skills.md)。新内容一律写进新篇；本篇只作历史参考，替代完成后删除。
>
> ⚠️ **已知与代码不符**：行动实体的归属已从 `ChildOf` 换成自定义关系
> `ActionOf` / `Actions`（M22，见 [relations.md](relations.md)）；本篇里凡是
> "行动实体用 `ChildOf` / 取 `parent()`"的说法都已过时（`unit_sprite` 与 HUD 的
> `ChildOf` 仍然有效——那是真的物理附着）。

> **描述对象：代码 A（仓库根 `src/`，package `app`）。**
> 本文按**依赖层次**自上而下排列：从没有游戏语义的引擎零件，到体素数据、
> 决策坐标、调度、战斗、行动载荷、表现，最后是**场景组合**。
> 目的是让「一个组件被谁创建、被谁读、被谁写、活多久」一眼可查。
>
> 配套阅读：[architecture.md](architecture.md)（分层与流水线） ·
> [timeline.md](timeline.md)（无回合语义）。

## 〇、读法

每一层只依赖它**上面**的层（编号更小的层），不反向依赖。表里的缩写：

- **创建** = 谁把这个组件挂到实体上（场景工厂 / 系统 / 组装层）；
- **读** = 查询里出现 `&T` 的系统；
- **写** = 查询里出现 `&mut T`，或用 `insert` / `remove` 改它的系统；
- 「生命周期」= 它什么时候消失。系统名按代码原名写，省略不了后缀的地方保留 `_system`。

```text
L7 场景组合      spawn::unit_scene -> player_scene / enemy_scene -> setup_scene
L6 表现与交互    CameraRig / UnitSprite / HUD 标记 / HoveredCell / 预演指示器
L5 行动载荷      MoveAction / JumpAction / RollAction / MeleeAction / ShootAction
                 FireballAction / ParryAction / Fireball
L4 战斗零件      Health / Faction / Collidable / 属性 / 防御标记 / 攻击实体生命周期
L3 时间线调度    DecisionSlot / ActionTiming / ScheduledAction / Uncancellable / InputDriven
L2 决策与位移    Cell / MoveGoal / Velocity / MoveSpeed / Jumping / DodgingOnArrival
L1 体素数据域    Chunk / ChunkPos / ChunkLoader / ChunkPinned / Voxel + 区块消息
L0 引擎零件      Transform / Visibility / Children / Mesh3d / Node / Text / Camera3d
```

## 一、L0 引擎零件（不属于任何领域）

这些是 Bevy 自带的组件 / 资源，项目只决定「谁写它」。**多个领域写同一个组件时，
顺序由 `configure_pipeline` 的系统集链保证**。

| 零件 | 谁写 | 谁读 |
| :--- | :--- | :--- |
| `Transform.translation` | `move_entities_system`（位移 + 到格吸附）、`jump_motion_system`（竖直）、`follow_terrain_system`（贴地）、`camera_*`（相机） | 几乎所有游戏系统（距离、射线、贴地、阴影、HUD） |
| `Transform.rotation` / `scale` | `billboard_system`（纸片偏航）、`update_preview_indicators_system`（扇形朝向）、`shadow_system`（阴影缩放）、`camera_*` | `lerp` 与渲染 |
| `GlobalTransform` | Bevy 的 transform propagation | `cursor_ray`、`billboard_system` |
| `Visibility` | `interaction`（高亮 / 预演）、`voxel_render` | Bevy 渲染 |
| `Children` / `ChildOf` | `unit_scene`（纸片 + 阴影）、`setup_hud`、**各行动场景工厂**（`ChildOf({actor})`） | `shadow_system` 用 `ChildOf` 找父单位；时间线系的执行器 / 撤销 / 打断 / HUD 用它取行动者 |
| `Mesh3d` / `MeshMaterial3d` | `interaction`（高亮、预演）、`voxel_render`（区块网格）、`spawn`（纸片 / 阴影）、`combat::skills`（攻击视觉） | Bevy 渲染 |
| `Mesh` / `StandardMaterial` / `Image` / `Font` 资产 | `setup_voxel_materials`、`spawn_hover_highlight`、`spawn_preview_indicators`、`load_unit_sprites`、`setup_hud` | 渲染 / 文本 |
| `Camera3d` / `MainCamera` / `IsDefaultUiCamera` | `presentation::camera::main_camera` | `cursor_ray`、`billboard_system` |
| `Node` / `Text` / `TextFont` / `TextColor` / `BackgroundColor` / `ImageNode` / `UiScale` / `Name` | HUD 各系统 | Bevy UI |
| `DirectionalLight` / `GlobalAmbientLight` | `setup_scene` / `preload` | 渲染 |
| `Timer` | `Lifetime` / `HintTimer` 等自有封装内部 | 对应系统 |

## 二、L1 体素数据域（`world` + `voxel_render`）

### 2.1 组件

| 组件 | 定义于 | 创建 | 读 | 写 / 生命周期 |
| :--- | :--- | :--- | :--- | :--- |
| `Chunk` | `world/chunk/components.rs` | `chunk_streaming_system`（空区块） | `generate_terrain_system`、`set_voxel`、`schedule_meshing_system`（clone 给后台） | `generate_terrain_system` 填地形、`set_voxel` 写字；区块实体销毁即消失 |
| `ChunkPos` | 同上 | 同上 | `generate_terrain_system`、`apply_meshing_result_system` | 不变 |
| `ChunkLoader { radius }` | 同上 | `player_scene` | `chunk_streaming_system` | 单位销毁即消失 |
| `ChunkPinned` | 同上 | **暂无系统创建**（预留：玩家改过的区块免卸载） | `chunk_streaming_system` | — |
| `Voxel` / `VoxelPos` | `world/voxel/components.rs` | **暂无系统创建**（预留：需要独立交互的方块实体） | 无 | — |
| `MeshingTask(Task<ChunkMeshes>)` | `voxel_render/meshing/components.rs` | `schedule_meshing_system` | `apply_meshing_result_system`（`poll_once`） | 完成后 remove；区块销毁 → 实体销毁 → 任务 drop（等于取消） |
| `ChunkSurface { chunk, voxel }` | 同上 | `apply_meshing_result_system` | `despawn_chunk_surfaces_system`（+ 重建时清旧网格） | 区块卸载 / 重建时销毁 |

### 2.2 资源与纯函数

| 资源 | 定义于 | 写 | 读 |
| :--- | :--- | :--- | :--- |
| `ChunkMap` | `world/storage/resources.rs` | `chunk_streaming_system` | `set_voxel` / `get_voxel` |
| `TerrainConfig` | `world/terrain/resources.rs` | 初始化（默认） | `generate_terrain_system`，以及 `ground_position` 的所有调用方：**movement**（到格吸附 / 贴地）、**interaction**（拾取、高亮、预演）、**spawn**（出生点）、**presentation**（阴影贴地） |
| `MeshingConfig` | `voxel_render/meshing/resources.rs` | 初始化 | `schedule_meshing_system`（随任务快照带走） |
| `VoxelMaterialRegistry` | `voxel_render/materials/resources.rs` | `setup_voxel_materials`（Startup） | `apply_meshing_result_system` |

```text
ChunkLoader -> chunk_streaming_system -> ChunkLoadEvent -> generate_terrain_system
                        |                                        |
                 （区块实体进 ChunkMap）                     ChunkDirtyEvent
                                                                 |
                                              schedule_meshing_system（异步）
                                                                 v
                                        apply_meshing_result_system -> ChunkSurface
ChunkUnloadEvent -> despawn_chunk_surfaces_system（清理网格）
```

**读这层的要点**：`world` 的地形高度是**纯函数**（`surface_height` /
`surface_height_at` / `ground_position`），调用方拿配置 + 坐标就能算，
不需要区块已加载。因此「单位贴地」不依赖流式加载状态——这是刻意的。

## 三、L2 决策坐标与位移（`movement`）

| 组件 | 定义于 | 创建 | 读 | 写 / 生命周期 |
| :--- | :--- | :--- | :--- | :--- |
| `Cell { x, z }` | `movement/cell.rs` | `unit_scene`（`Cell::from_world(出生位置)`） | **决策系统的通用语言**：movement 的声明系统、`move_action_executor_system`、`ai::enemy_declare_system`、`combat::defense::declare_roll_system`、`combat::reaction`（威胁按格声明、玩家站在哪格）、`interaction`（悬停色 / 点击判定）、`presentation::hud::panels` | **只有 `move_entities_system` 写**（吸附到格中心那一刻） |
| `MoveGoal { cell }` | 同上 | `move_action_executor_system`、`roll_executor_system` | `move_entities_system` | 到格中心时 remove |
| `Velocity(Vec3)` | `movement/components.rs` | `unit_scene`（零）、`arrow_scene`、`fireball_scene` | `move_entities_system`、`combat::defense` 的攻击探针 | `move_action_executor_system` / `roll_executor_system`（起步）、`move_entities_system`（每帧位移）、`apply_physical_hits_system`（命中后归零）、`projectile_arrival_system`（到达归零） |
| `MoveSpeed(f32)` | 同上 | `player_scene`(5.0) / `enemy_scene`(2.0) | `move_action_executor_system`（算 `Velocity`，并据此估出「还要走多久」`effect_delay`） | 不变 |
| `Jumping { ground_y, velocity }` | `movement/actions.rs` | `jump_action_executor_system` | `jump_motion_system`、`follow_terrain_system`（过滤）、HUD 面板（状态行） | `jump_motion_system` 落地时 remove |
| `DodgingOnArrival { expires_at }` | `movement/systems.rs` | `roll_executor_system` | `move_entities_system` | 到位那一刻**换成 `Dodging`** 并 remove |

**为什么 `DodgingOnArrival` 要单独存在**：无敌帧必须和位移**同时**生效。
提前挂会在原地就无敌，推迟挂会在飞出去后留破绽，所以它跟着 `MoveGoal`
走，由 `move_entities_system` 在吸附到位那一刻兑现。

## 四、L3 时间线调度（`timeline`）

### 4.1 组件

| 组件 | 挂在哪 | 创建 | 读 | 写 / 生命周期 |
| :--- | :--- | :--- | :--- | :--- |
| `DecisionSlot`（`Empty` / `Windup` / `Recovery`） | 单位 | `unit_scene`（`Empty`） | movement / combat / defense / skills 的**所有**声明系统、`ai` 两个系统、`menu` 的循环与派发、`compute_player_awaiting_system`（世界要不要停）、`combat::reaction`（玩家表态了没有）、HUD（面板状态行、时间轴候场区） | 声明时 `Windup`（`add_child` 行动实体 + insert）；`recovery_system` 在后摇到点时置 `Empty`；`undo_system` / `interrupt_observer` 置 `Empty` |
| `InputDriven` | 单位 | `player_scene` | `compute_player_awaiting_system`（停下等谁）、`combat::reaction`（谁被威胁 / 谁在表态）、`undo_system`（只撤玩家的行动）、各玩家声明系统（认人不认阵营） | 不变 |
| `ScheduledAction { execute_at }` | 行动实体（行动者的**子实体**，`ChildOf`） | 所有行动场景工厂（`declared_at` / `immediate` / `with_focus`） | 各执行器（`due`）、`undo_system` / `interrupt_observer`（`pending`）、`ai::decide_intent_system`（威胁）、HUD 时间轴 / 行动行 | 不变（行动实体销毁即消失）；**只有"什么时候落地"一个事实**：归属归 `ChildOf`、节奏归 `ActionTiming` |
| `Uncancellable` | 行动实体 | 行动场景工厂（目前只有 `jump_action_scene`） | `undo_system`（`Without<Uncancellable>`：挂了就不给撤） | 行动实体销毁即消失；退款金额不在这里（见 5.4 的退款 Observer） |
| `ActionTiming { windup, recovery, interrupt_resist }` | 行动实体（跟着载荷一起挂） | 各领域自己的 `*_TIMING` 常量，由行动场景工厂挂上 | 各执行器（算忙碌窗口）、HUD 时间轴（`execute_at − windup` 起算、宽度 `total()`）、`combat::formula::interrupt_observer`（守方底数） | 不变；**具体数值归载荷**（见 [timeline.md](timeline.md) 第三节） |

### 4.2 资源

| 资源 | 写 | 读 |
| :--- | :--- | :--- |
| `PauseReasons(HashSet<&'static str>)` | `process_pause_requests`（帧末每帧重建：先 `clear()`，再按消息顺序放断言） | `apply_clock`（唯一的停表判据）、`input::keyboard::pause_input_system`（手动暂停开着没有）、HUD 时间轴（冻结原因） |
| `Focus { current, max }` | `recover_focus_system`（+1）、各玩家声明系统（花 1 点换零前摇） | 同左 |
| `FocusIntent(bool)` | `track_focus_intent_system` | 各玩家声明系统（本帧要不要用 Focus） |

### 4.3 系统链（顺序即语义）

```text
compute_player_awaiting_system 玩家决策槽空着 -> 每帧断言 "slot_empty"
track_focus_intent_system      Shift + 决策键 -> FocusIntent（只在本帧有效）
undo_system                    右键 / 换手（PlayerIntent）：销毁还没到点的玩家行动 + 清空决策槽 + trigger ActionCancelled
recovery_system                后摇到点 -> 决策槽置 Empty + trigger DecisionReady
recover_focus_system           每 10 虚拟秒回 1 点 Focus

（帧末 ClockSet）process_pause_requests -> apply_clock：**唯一**写 Time<Virtual> 的地方
```

`FirstReady::first_ready` 不是系统，而是**声明系统共用的入口**：迭代器上的一个方法，
挑出此刻能决策的行动者（判据只有「槽是 `Empty`」一份），挑不到就写一条 `ActionBlocked::BUSY`。
哪个分量是决策槽由 `HasDecisionSlot` 回答，因此调用点只要
`players.iter().first_ready(&mut blocked)`（约定：**决策槽放在查询元组末位**）。

`interrupt_observer`（住 `combat::formula`）/ `recover_stamina_observer` / 两个退款 Observer 都不在系统链里：
它们是 EntityEvent 的 Observer，触发那一刻当场执行（见 [timeline.md](timeline.md) 第四 / 五节）。

手动暂停不在这一域：`input::keyboard::pause_input_system` 读 `PauseReasons` 判断
"按一下是暂停还是恢复"，然后每帧断言（或不再断言）`"manual"`。

**没有集中式收尾函数**：每个执行器自己判 `if !schedule.due(now) { continue; }`、
自己销毁行动实体、自己给行动者写 `DecisionSlot::recovering(..)`
（往可能已经阵亡的行动者上写命令要先 `get_entity` 守卫）。

## 五、L4 战斗零件（`combat`）

### 5.1 身份与资源

| 组件 | 创建 | 读 | 写 |
| :--- | :--- | :--- | :--- |
| `Faction` | `unit_scene` | 几乎所有战斗系统（目标过滤 / 敌我判定）、`ai`、`combat::reaction`（威胁必须来自敌对阵营）、`presentation`（立绘、面板、日志、相机跟随） | 不变 |
| `Collidable` | `unit_scene` | `detect_collisions_system`、`detect_melee_system`、`reset_battle_system` | 不变 |
| `Health { current, max }`（整数） | `unit_scene`(50) | `ai::decide_intent_system`（血量比例）、`explosion_system`（`With<Health>` 过滤）、HUD 面板 | `apply_damage_system`（**唯一**扣血入口）、`despawn_dead_system`（`current <= 0` 时销毁） |
| `Stamina` | `unit_scene`（默认 5） | `declare_roll_system` / `declare_parry_system`（够不够）、`declare_fireball_system`（够不够）、`ai::enemy_declare_system`、`menu` 的可用性判断、HUD 技能栏 / 面板 | 扣费：`roll_executor_system`(1) / `parry_executor_system`(1) / `declare_fireball_system`(2)；回复：`recover_stamina_observer`（订阅 `DecisionReady`，+1）、`refund_fireball_observer` / `refund_melee_observer`（撤销退款） |

### 5.2 数值属性（`combat/attributes`）

| 组件 | 谁挂 | 读 |
| :--- | :--- | :--- |
| `PhysicalDamage(i32)` | `melee_scene`(15) / `arrow_scene`(10) / `fireball_scene`(12) | `apply_physical_hits_system`（护甲减免后写成 `DamageEvent`） |
| `AttackFrame(u32)` | 同上（5 / 4 / 7） | 暂无消费者——它是**信息层**读数（"谁先动"），见 [../TODO.md](../TODO.md) 的「洞察力」 |
| `InterruptPower(i32)` | 同上（3 / 1 / 2） | `apply_physical_hits_system`（命中时触发 `InterruptEvent`） |
| `AttackRange` | `unit_scene`（`MELEE` = 1 格）；攻击实体**不挂** | `ai::decide_intent_system`（射程内 → `Shoot`） |
| `HitRadius` | `unit_scene`(0.8) / `arrow_scene`(0.2) / `fireball_scene`(0.35) | `detect_collisions_system`（距离 ≤ 两者半径之和） |
| `Armor(i32)` | **只有测试挂**（`armor_reduces_physical_damage`）；组装层还没给任何单位护甲 | `apply_physical_hits_system`（`physical_damage` 减免） |

> `AttackRange` 是**声明在单位身上**的武器属性；`PhysicalDamage` / `AttackFrame` /
> `InterruptPower` 是**声明在攻击实体身上**的一次性数值。
> **伤害类型不是枚举**：加一种元素伤害 = 加一个组件 + 加一个同形的命中系统。

### 5.3 防御与攻击实体

| 组件 | 创建 | 读 | 写 / 生命周期 |
| :--- | :--- | :--- | :--- |
| `Dodging { expires_at }` | `move_entities_system`（兑现 `DodgingOnArrival`） | `apply_physical_hits_system`（`DefenseState`）、HUD 面板 | `expire_defense_markers_system`（虚拟时间到点 remove） |
| `Parrying { target_attack, expires_at }` | `parry_executor_system` | `apply_physical_hits_system`（招架判定）、HUD 面板 | `expire_defense_markers_system`（到点 **或绑定攻击消失**时 remove） |
| `Projectile { max_hits, current_hits, finished }` | `arrow_scene`(1) / `fireball_scene`(0) | `detect_collisions_system`（`finished` 跳过）、`apply_physical_hits_system`（同上）、`cleanup_finished_attacks_system`、防御探针 | `apply_physical_hits_system`（+1 命中 / 标 finished）、`projectile_arrival_system`（换成 finished） |
| `HitOnce { spent }` | `melee_scene` | `detect_melee_system`、`apply_physical_hits_system` | 命中后置 `spent` |
| `Lifetime(Timer)` | `melee_scene`（0.18s） | `expire_attack_entities_system`、防御探针 | 到期 despawn |
| `MeleeShape { range, half_arc }` | `melee_scene`（2.5 / 60°） | `detect_melee_system`、防御探针 | 不变 |
| `CollisionTarget(Entity)` | `detect_collisions_system` / `detect_melee_system` | `apply_physical_hits_system`、`declare_parry_system`（找"正打向我的那次攻击"） | 每帧先清 stale（碰撞检测里）、结算后 `apply_physical_hits_system` remove |
| `Threatens { cells }` | `fireball_action_scene`（飞行路径）/ `melee_action_scene`（正前方 + 两侧） | `detect_threat_system`（威胁是否压在玩家格上）、`ai::decide_intent_system`（是否压在自己格上） | 行动实体销毁即消失 |
| `TargetCell(Cell)` | `fireball_scene`（飞行中的投射物） | `projectile_arrival_system`（到达判定）、`detect_threat_system`（飞行中也是威胁） | 到达时 remove（它不再是威胁） |

### 5.4 战斗资源与撤销退款

| 资源 | 写 | 读 |
| :--- | :--- | :--- |
| `ThreatWindow { threatening, opening_action, answered }` | `detect_threat_system` | 同系统（一次威胁只开一个窗口） |
| `MenuSelection { index }` | `select_skill_system` / `cycle_skill_system` | `use_selected_skill_system`、`interaction`（预演指示器）、HUD 技能栏 |

**撤销退款是 Observer，不是资源操作**（都不在系统链里，`ActionCancelled` 触发即执行）：

| Observer | 订阅 | 做什么 |
| :--- | :--- | :--- |
| `combat::skills::fireball::refund_fireball_observer` | `ActionCancelled` + `With<FireballAction>` | 退 `FIREBALL_COST`(2) 再收 2：净额不变，撤销本身要有分量 |
| `combat::skills::actions::refund_melee_observer` | `ActionCancelled` + `With<MeleeAction>` | 收 `MELEE_CANCEL_PENALTY`(1)：抡出去再收招 |
| `combat::defense::recover_stamina_observer` | `DecisionReady` | 后摇结束、重新可决策 → +1 精力 |

翻滚 / 招架**不退款**：它们的精力在执行时才扣（`roll_executor_system` /
`parry_executor_system`），前摇里撤销时还没花过钱。

### 5.5 `CombatSet` 内部链

```text
detect_threat_system              敌对威胁压在玩家格上 -> 每帧断言冻结（本帧末生效）
expire_defense_markers_system     本帧到期的无敌帧不该再生效
select_skill_system / cycle_skill_system
use_selected_skill_system         按选择派发成 FireCommand / MeleeCommand / RollCommand
declare_fireball_system / declare_melee_system / declare_roll_system / declare_parry_system
melee / fireball / roll / parry 执行器    到点落地 -> 销毁行动实体 + 写 DecisionSlot::Recovery（各自收尾）
projectile_arrival_system -> explosion_system
detect_collisions_system / detect_melee_system    挂 CollisionTarget
apply_physical_hits_system        防御判定 + 护甲 + 打断触发 + 命中计数
apply_damage_system               唯一扣血点（死亡只报一次）
cleanup_finished_attacks_system / expire_attack_entities_system
despawn_dead_system               帧末：Health <= 0 的实体销毁
```

## 六、L5 行动载荷（载荷 + 工厂 + 执行器）

每个动作 = **一个载荷组件 + 一个场景工厂 + 一个执行器**。载荷住在它归属的领域，
调度器完全不认识它们。

| 载荷 | 定义于 | 工厂 | 声明的触发源 | 执行器 | 到点做什么 |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `MoveAction { from_cell, to_cell }` | `movement/actions.rs` | `move_action_scene` | `MoveCommand`（方向键）、`MoveToCommand`（点地板）、`ai` 的 `Approach` / `Retreat` | `move_action_executor_system` | 朝向目标格中心设 `Velocity` + 挂 `MoveGoal`；忙到走到位 |
| `JumpAction` | 同上 | `jump_action_scene` | `JumpCommand`（`C`） | `jump_action_executor_system` | 挂 `Jumping`（向上初速度） |
| `RollAction { from_cell, to_cell }` | 同上 | `roll_action_scene` | `RollCommand`（`E`）、`ai::declare_roll` | `roll_executor_system` | 扣 1 精力 + 设速度 + 挂 `MoveGoal` + `DodgingOnArrival` |
| `MeleeAction` | `combat/skills/actions.rs` | `melee_action_scene` | `MeleeCommand`（`W` / 菜单派发） | `melee_action_executor_system` | 朝最近敌人生成 `melee_scene` 攻击实体 |
| `ShootAction` | 同上 | `shoot_action_scene` | **未注册**（`declare_skill_system` 不在插件里） | `shoot_action_executor_system` | 生成 `arrow_scene` 箭矢（保留为单体狙击的参考实现） |
| `FireballAction { target_cell }` | `combat/skills/fireball.rs` | `fireball_action_scene` | `FireCommand`（`Q` / 菜单派发）、`ai::declare_fireball_at` | `fireball_action_executor_system` | 从当前站位生成 `fireball_scene` 投射物；忙到飞行结束 |
| `ParryAction { target_attack }` | `combat/defense/components.rs` | `parry_action_scene` | `ParryCommand`（`R`） | `parry_executor_system` | 扣 1 精力 + 挂 `Parrying` |
| `Fireball { speed, amount, radius }` | `combat/skills/fireball.rs` | `fireball_scene` | 火球执行器 | （数据 + `projectile_arrival_system`） | 到格 → 广播 `ProjectileArrived`（目标格住在 `TargetCell` 上） |

**非载荷但同属这一层的状态**：`Jumping`（弹道，`jump_motion_system`）、
`DodgingOnArrival`（到位兑现）、`Fireball`（投射物数据）。

## 七、L6 表现与交互零件

### 7.1 `interaction`（鼠标拾取与预演）

| 零件 | 类型 | 创建 | 读 | 写 |
| :--- | :--- | :--- | :--- | :--- |
| `HoveredCell(Option<Cell>)` | 资源 | 初始化 | `update_hover_highlight_system`、`update_preview_indicators_system`、`update_preview_readout_system`、`pointer_command_system` | `hover_cell_system`（光标 → 射线 → 格） |
| `HoverHighlight` | 组件 | `spawn_hover_highlight`（Startup 一个） | `update_hover_highlight_system` | 不增删，只搬位置 / 改色 |
| `HoverTint(Color)` | 组件 | 同上 | `update_hover_highlight_system` | 同系统（变了才写材质） |
| `AoePreview` | 组件 | `spawn_preview_indicators` | `update_preview_indicators_system` | 同系统（位置 + 可见性） |
| `ConePreview` | 组件 | 同上 | 同上 | 同上 |

拾取是**纯函数**：`cursor_ray`（相机 → 射线）+ `pick_cell`（沿高度场步进），
因此可以脱离渲染单测。`HoveredCell` 存成资源而不是组件：它是「这一帧的输入状态」，
只可能有一份，而且注册进反射后 BRP 能直接读。

### 7.2 `presentation`（相机 / 纸片 / 装饰 / 日志 / HUD）

| 零件 | 类型 | 创建 | 读 | 写 |
| :--- | :--- | :--- | :--- | :--- |
| `MainCamera` | 组件 | `main_camera` | `cursor_ray`（interaction）、`billboard_system` | 不变 |
| `CameraRig { focus, offset, bounds, pan_offset, follow_rate, zoom }` | 组件 | `main_camera` | `camera_pan_system` / `camera_zoom_system` / `camera_follow_system` | 同三个系统（平移改 `pan_offset`、缩放改 `zoom`、跟随改 `focus`） |
| `UnitSprite` | 组件 | `unit_scene`（子实体） | `billboard_system` | `billboard_system`（偏航角） |
| `UnitShadow` | 组件 | `unit_scene`（子实体） | `shadow_system` | `shadow_system`（贴地 + 随高度收缩） |
| `UnitSprites { player, enemy, shadow }` | 资源 | `load_unit_sprites`（Startup / `PreloadSet`） | `spawn`（组装单位）、`setup_hud`（头像）、`tests/assets.rs` | 不变 |
| `Natures { models }` | 资源 | `load_natures`（Startup） | `setup_scene`（`random()` 造装饰场景） | 不变 |
| `BattleLog { entries, max }` | 资源 | 初始化 | `update_log_panel_system` | `battle_log_system`（读 `DamageEvent` / `DeathEvent`） |
| `HudCache` | 资源 | 初始化 | 所有 HUD 更新系统（快照比对） | 同左（内容真的变了才碰 UI 节点） |
| `HintTimer(f32)` | 资源 | 初始化 | `update_action_hint_system` | 同系统（走 `Time<Real>`，冻结时也能淡出） |

HUD 标记组件（`HudRoot` / `PanelBar` / `PanelText` / `ActionLabel` / `SkillSlot` /
`SkillBadge` / `SkillTooltip` / `Timeline*` / `Log*` / `HelpPanel` / `ActionHint`）由
`setup_hud` 及各 `spawn_*` 创建，各自的更新系统只改 `Node` / `Text` / `BackgroundColor`。

HUD 只读游戏状态：`update_unit_panels_system` 读 `Health` / `Stamina` / `Cell` /
`DecisionSlot` / `Dodging` / `Parrying` / `Jumping` / `Intent`；
`update_timeline_system` 读 `ScheduledAction` / `ActionTiming` / `DecisionSlot` / `Faction` / `PauseReasons`；
`update_action_labels_system` 读七种载荷组件（判断「当前行动是什么」）；
`update_skill_bar_system` 读 `MenuSelection` 与玩家 `Stamina`。

### 7.3 `input`（只翻译）

| 零件 | 类型 | 读 | 写 |
| :--- | :--- | :--- | :--- |
| `HotkeyBinds { entries }` | 资源 | `player_skill_input_system` | 初始化（默认 `Q` 火球 / `W` 近战 / `E` 翻滚 / `R` 招架） |

键盘系统读 `ButtonInput<KeyCode>` 与相机 `Transform`（算屏幕基），**只写消息**；
鼠标系统读 `ButtonInput<MouseButton>` / `AccumulatedMouseMotion` / `MouseWheel`，同样只写消息。

## 八、L7 场景组合（`spawn`）

### 8.1 单位实体树

```text
unit_scene(faction, position, sprites)                    <- 共用零件
|-- Faction / Health(50) / HitRadius(0.8) / AttackRange::MELEE / Collidable
|-- Velocity(ZERO) / DecisionSlot::Empty / Stamina(default 5) / Cell::from_world(position)
|-- Transform { translation: position }   <- 脚底、无旋转、无缩放
|-- Visibility                            <- 子节点带可见性，父节点必须有同名组件（否则 B0004）
`-- Children
    |-- UnitSprite + Mesh3d(1.8^2) + 材质(阵营贴图) + Transform{ y = 0.9 }
    `-- UnitShadow + Mesh3d(1.9^2) + 半透明黑 + 绕 X 转 -90°（平铺）

player_scene(terrain, sprites) = unit_scene(Player, 格 (1,0) 中心)
                               + InputDriven（输入归属）+ MoveSpeed(5.0) + ChunkLoader
enemy_scene(terrain, sprites)  = unit_scene(Enemy, 格 (3,3) 中心)
                               + MoveSpeed(2.0) + EnemyBrain（-> #[require(Intent)]）
```

**为什么单位根节点必须是「脚底 + 无旋转 + 无缩放」**：纸片和阴影是它的子节点，
子节点的**局部坐标**要等于世界偏移，才能用 `Vec3::Y` 直接表达「抬高多少」「阴影贴在地表下多少」。
视觉尺寸放在网格上，不放在缩放上。

### 8.2 开局与重置

| 系统 | 时机 | 做什么 |
| :--- | :--- | :--- |
| `preload`（`PreloadSet`） | Startup，最早 | 插 `GlobalAmbientLight`、`Natures`、`UnitSprites` |
| `setup_hud`（`.after(preload)`） | Startup | 建整套 HUD（用单位精灵当头像，所以必须在预载之后） |
| `setup_scene`（`AssemblySet`） | Startup | 方向光 → 相机 → 玩家 → 敌人 → 5×5 装饰 |
| `restart_input_system`（`InputSet`）→ `reset_battle_system`（`SpawnSet`） | Update | `F5` → `ResetBattle` → 清单位 / 攻击实体 → 用同一组工厂重建 |

`F5` 的读取住在 `input::keyboard::restart_input_system`：组装车间**不认识按键**，
它只消费 `ResetBattle`。清场查询只需要 `Faction` / `Projectile` / `Collidable`——
没执行的行动是单位的子实体，人没了行动跟着没（`Children` 是 linked spawn）。

**攻击实体不在组装层**：它们是技能的产物，工厂归 `combat::skills`
（`melee_scene` / `arrow_scene` / `fireball_scene`）。

### 8.3 完整依赖链（从零件到场景）

```text
TerrainConfig --> cell_ground --> player_scene / enemy_scene
UnitSprites   --> unit_scene  --^
                        |
                        v
              DecisionSlot（能不能决策）+ Cell（决策坐标）+ Faction（身份）
              InputDriven（决策来自玩家输入，只有玩家有）
                        |
      +-----------------+----------------------+
      v                 v                      v
  input/ai 声明行动   时间线：决策槽三态 / 暂停原因      HUD / 相机只读
      |                 |
      `-> 行动实体（载荷 + ScheduledAction + Uncancellable）--ChildOf(actor) 由工厂写进模板--> 行动者
                              |
                              v
                          执行器 -> 攻击实体
                                      |
          targeting -> apply_physical_hits -> apply_damage -> despawn_dead
```

## 九、系统 × 组件反查（按系统集）

| 系统 | 读 | 写 |
| :--- | :--- | :--- |
| `compute_player_awaiting_system` | `DecisionSlot`、`InputDriven` | `PauseRequest`（每帧断言 `"slot_empty"`） |
| `track_focus_intent_system` | `UseFocus` | `FocusIntent` |
| `undo_system` | `UndoCommand`（右键）、`PlayerIntent`（换手，两者在这一条系统里汇合）、`ScheduledAction`（`pending`）、`ChildOf`（取行动者）、`Uncancellable`（`Without`）、`InputDriven` | trigger `ActionCancelled`、销毁行动实体、`DecisionSlot::Empty` |
| `recovery_system` | `DecisionSlot`（只处理 `Recovery`）、`Time<Virtual>` | `DecisionSlot::Empty`、trigger `DecisionReady` |
| `recover_focus_system` | `Time<Virtual>` | `Focus` |
| `process_pause_requests` / `apply_clock` | `PauseRequest` / `PauseReasons` | `PauseReasons`（每帧 `clear` 后重建）/ `Time<Virtual>`（**唯一**写时钟的地方） |
| `interrupt_observer`（住 `combat::formula`） | `InterruptEvent`、`ScheduledAction`（`pending`）、`ActionTiming`（守方 `interrupt_resist`）、`ChildOf`（取行动者）、`Time<Virtual>` | 销毁行动实体、`DecisionSlot::Empty` |
| `declare_move_system` / `declare_move_to_system` / `declare_jump_system` | 命令消息、`InputDriven`、`DecisionSlot`、`Cell`、`Focus`、`FocusIntent`、`Time<Virtual>` | 行动实体、行动者的 `DecisionSlot::Windup` + `ChildOf`、`ActionBlocked` |
| `move_action_executor_system` | `MoveAction`、`ScheduledAction`（`due`）、`ActionTiming`（忙碌窗口）、`ChildOf`、`Cell`、`MoveSpeed`、`Transform` | `Velocity`、`MoveGoal`、`DecisionSlot::Recovery`、销毁行动实体 |
| `move_entities_system` | `Transform`、`Velocity`、`MoveGoal`、`DodgingOnArrival`、`Time`、`TerrainConfig` | `Transform`、`Velocity`、`Cell`、`Dodging`、`MoveGoal`(remove) |
| `follow_terrain_system` | `Transform`、`Cell`、`Jumping`(排除)、`TerrainConfig` | `Transform.y` |
| `jump_motion_system` | `Jumping`、`Time` | `Transform.y`、`Jumping`(remove) |
| `decide_intent_system` | `Transform`、`Cell`、`Faction`、`Health`、`AttackRange`、`EnemyBrain`、`DecisionSlot`、`Threatens` | `Intent` |
| `enemy_declare_system` | `Intent`、`DecisionSlot`、`Cell`、`Faction`、`Stamina`、`Transform` | 行动实体（工厂自带 `ChildOf`）、`DecisionSlot::Windup` |
| `select_skill_system` / `cycle_skill_system` | `SelectSkill` / `CycleSkill`、`Stamina`（`InputDriven`）、`DecisionSlot` | `MenuSelection` |
| `use_selected_skill_system` | `UseSelectedSkill`、`MenuSelection`、`Stamina`、`DecisionSlot`、`InputDriven`、`Transform`、`Faction` | `FireCommand` / `MeleeCommand` / `RollCommand`、`ActionBlocked` |
| `declare_fireball_system` | `FireCommand`、`InputDriven`、`DecisionSlot`、`Cell`、`Stamina`、`Transform`、`Faction`、`Focus`、`FocusIntent` | 行动实体（带 `Threatens`）、`Stamina`、`DecisionSlot::Windup` + `ChildOf` |
| `declare_melee_system` / `declare_roll_system` / `declare_parry_system` | 对应命令、`InputDriven`、`DecisionSlot`、`Cell`/`Stamina`/`Transform`/`Faction`、`Focus`、`FocusIntent`、`CollisionTarget`(招架找威胁) | 行动实体、`DecisionSlot::Windup` + `ChildOf`、`Stamina`(翻滚 / 招架仅在执行时) |
| `fireball_action_executor_system` | `FireballAction`、`ScheduledAction`（`due`）、`ActionTiming`（忙碌窗口）、`ChildOf`、`Transform`、`Faction` | 火球实体、`DecisionSlot::Recovery`、销毁行动实体 |
| `roll_executor_system` | `RollAction`、`ScheduledAction`（`due`）、`ActionTiming`（忙碌窗口）、`ChildOf`、`Velocity`、`Stamina`、`Transform` | `Velocity`、`Stamina`、`MoveGoal`、`DodgingOnArrival`、`DecisionSlot::Recovery` |
| `parry_executor_system` | `ParryAction`、`ScheduledAction`（`due`）、`ActionTiming`（忙碌窗口）、`ChildOf`、`Stamina` | `Parrying`、`Stamina`、`DecisionSlot::Recovery` |
| `melee_action_executor_system` | `MeleeAction`、`ScheduledAction`（`due`）、`ActionTiming`（忙碌窗口）、`ChildOf`、`Transform`、`Faction` | 近战攻击实体、`DecisionSlot::Recovery` |
| `detect_threat_system` | `ScheduledAction`（`pending`）、`Threatens`、`TargetCell`、`Cell`、`Faction`、`InputDriven`、`ChildOf`（取行动者）、`Time<Virtual>` | `ThreatWindow`、`PauseRequest`（每帧断言 `"threat"`） |
| `projectile_arrival_system` | `Fireball`、`TargetCell`、`Transform`、`Velocity`、`Faction` | `Transform`、`Velocity`、`Projectile`、`ProjectileArrived`、remove `TargetCell`/`Fireball` |
| `explosion_system` | `ProjectileArrived`、`Transform`、`Faction`、`Health` | `DamageEvent`、销毁投射物 |
| `detect_collisions_system` | `CollisionTarget`、`Transform`、`HitRadius`、`Projectile`、`Faction`、`Collidable` | `CollisionTarget`（先清后挂） |
| `detect_melee_system` | `Transform`、`Faction`、`MeleeShape`、`HitOnce`、`Collidable` | `CollisionTarget`、`HitOnce.spent` |
| `apply_physical_hits_system` | `PhysicalDamage`、`CollisionTarget`、`InterruptPower`、`Armor`、`Dodging`、`Parrying`、`Projectile`、`HitOnce` | `DamageEvent`、`InterruptEvent`（trigger）、`Projectile`、`HitOnce`、remove `CollisionTarget` |
| `apply_damage_system` / `despawn_dead_system` | `DamageEvent` / `Health` | `Health`、`DeathEvent` / 销毁实体 |
| `expire_defense_markers_system` | `Dodging`、`Parrying`、攻击探针（`Projectile`/`Velocity`/`MeleeShape`/`Lifetime`）、`Time<Virtual>` | 移除到期标记 |
| `refund_fireball_observer` / `refund_melee_observer` | `ActionCancelled`（EntityEvent）、`FireballAction` / `MeleeAction` | `Stamina`（退 / 收的手续费各自由花钱的领域决定） |
| `recover_stamina_observer` | `DecisionReady`（EntityEvent） | `Stamina`（+1） |
| `cleanup_finished_attacks_system` / `expire_attack_entities_system` | `Projectile` / `Lifetime`、`Time` | 销毁攻击实体 |
| `hover_cell_system` | `Window`、`Camera`、`GlobalTransform`、`TerrainConfig` | `HoveredCell` |
| `pointer_command_system` | `PointerCommand`、`HoveredCell`、`Cell`、`Faction` | `MoveToCommand` / `UseSelectedSkill` / `UndoCommand` / `PlayerIntent` |
| `pause_input_system` / `restart_input_system` | `ButtonInput<KeyCode>`、`PauseReasons`（手动暂停开着没有） | `PauseRequest` / `ResetBattle` |
| `camera_*` | `PanCamera` / `ZoomCamera`、`Transform`、`Faction`、`Time<Real>` | `CameraRig`、相机 `Transform` |
| `battle_log_system` | `DamageEvent`、`DeathEvent`、`Faction` | `BattleLog` |
| `update_*_system`（HUD） | 见第七节 | 只写 UI 组件 |
| `chunk_streaming_system` | `Transform`、`ChunkLoader`、`ChunkPinned`、`ChunkMap` | 区块实体、`ChunkMap`、三条区块消息 |
| `generate_terrain_system` | `ChunkLoadEvent`、`TerrainConfig` | `Chunk`、`ChunkDirtyEvent` |
| `schedule_meshing_system` | `ChunkDirtyEvent`、`Chunk`、`MeshingConfig` | `MeshingTask` |
| `apply_meshing_result_system` | `MeshingTask`、`ChunkPos`、`VoxelMaterialRegistry`、`ChunkSurface` | 网格实体、`ChunkSurface`、`MeshingTask`(remove) |
| `despawn_chunk_surfaces_system` | `ChunkUnloadEvent`、`ChunkSurface` | 销毁网格实体 |

## 十、设计观察（review 发现，供决策）

这些是读码时发现的**结构性问题**，不是 bug 报告；每条给出落点与影响。

### 10.1 玩家身份：`InputDriven` 收口（本次已落地）

「谁是玩家」现在是**一个标记组件** [`InputDriven`](timeline.md)（组装层挂在玩家身上）。
所有声明系统、时间线与反应系统都认它，不再满世界
`find(|faction| *faction == Faction::Player)`：

- `Faction` 管**战斗目标过滤**（谁能打谁、谁被谁威胁）；
- `InputDriven` 管**输入归属**（世界停下来等谁、谁能撤销、谁被保护）。

两者语义不同、互不替代：将来加「第二个玩家」「被 AI 接管的角色」时，
换的只是驱动源那一个组件。仍按 `Faction::Player` 找人的地方只剩下
**表现层**（面板 / 头像 / 相机跟随），它们看的是"哪个阵营的单位"，不是"谁在输入"。

### 10.2 重复实现与无人消费的出口

| 现象 | 位置 | 说明 |
| :--- | :--- | :--- |
| `declare_skill_system` / `shoot_action_*` / `arrow_scene` | `skills/actions.rs`、`skills/arrow.rs` | 未注册，只被测试使用（「单体狙击」的预留实现）。箭矢的收尾已经和火球对齐（忙到落地），接输入即可用 |
| `AttackFrame` | `combat/attributes` | 有生产者（攻击实体都挂）、没有消费者：它是**信息层**读数（"谁先动"），等「洞察力」面板来接 |
| `Voxel` / `VoxelPos` / `ChunkPinned` | `world` | 有类型、有文档、没有生产者 |

处理建议：要么接上（箭矢接输入、`AttackFrame` 给洞察力面板），要么删掉，
不要让「看起来在用」的代码留在树里。

> 本轮重构**已删除**：`expire_defense_markers_system` 的重复实现、
> `manage_projectile_hits_system`、`AttackResolved`（三者都是"写了没人读"的死代码）。

### 10.3 文档与代码不一致的地方（历次已修正）

重写文档时发现旧文档里的下列描述**与代码不符**，新文档已按代码改正：

| 旧文档的说法 | 代码实际 |
| :--- | :--- |
| 「等 `Enter` 确认 / `require_commit`」 | 声明即生效（`ScheduledAction` 只认时间戳）；反悔走撤销 / 打断 |
| 火球「投射物在声明时就生成」 | 执行器发射时生成（撤销不会留下半空火球） |
| 前摇中的行动会冻结世界 | 冻结只看 `PauseReasons`（等输入 / 手动 / 威胁），**没有**读某条行动的状态 |
| AI 的 `Dodge` 写玩家的 `RollCommand` | AI 直接 `declare_roll`，`RollCommand` 只属于 PC |
| `ChunkLoader` 玩家半径是 3×3 区块 | 默认 `radius = (0,1,0)`：XZ 只加载 1×1 区块 |
| HUD 缓存文档里的「WeGo 冻结」 | 无回合模型（措辞遗留，不影响行为） |
| 时间线三层状态机（`Declared`/`Pending`/`Committed`）、`Arbitration` 两阶段结算 | 全部删除，改为「决策槽三态 + 时间戳 + 事件」模型（见 [timeline.md](timeline.md)） |
| 后摇是行动者身上的 `Busy` 组件、取消代价是行动上的 `Cancellable` 枚举 | 后摇是决策槽的 `Recovery { until }`；"能不能撤"只剩 `Uncancellable` 标记，退多少钱归花钱的领域 |
| `ScheduledAction.actor` 字段 | 行动是行动者的子实体（`ChildOf`），调度数据里不再抄一份归属 |
| 手动暂停是 `ManualPause` 资源 + 边沿触发的 `compute_manual_pause` | "按一下是暂停还是恢复"住在 `input::keyboard::pause_input_system`（读 `PauseReasons`），暂停原因每帧重新断言 |

### 10.4 值得保留的设计（别在重构时弄丢）

1. **`move_entities_system` 是唯一写位移与 `Cell` 的地方**——
   这个「一个系统管一类实体的位移」让移动 / 投射物 / 跳跃不会互相争抢。
2. **`DodgingOnArrival` 把「无敌帧」和「位移到位」绑在一起**——
   它用一个短命组件换掉了「在哪一帧挂标记」的隐式约定。
3. **`DecisionSlot::recovering(timing, now, effect_delay)` 把「忙到效果发生」变成显式参数**——
   这是「按了技能没放出去」这类 bug 的通用解法，新动作要照抄
   （移动忙到走到格中心、火球忙到落地、箭矢忙到命中）。
4. **暂停用「原因集合」而不是一个布尔**——`"manual"` / `"slot_empty"` / `"threat"`
   可以叠加，谁也不覆盖谁；唯一写时钟的地方是帧末的 `apply_clock`。
5. **`world` 完全不引用渲染类型**——它换来了「数据域可以 `MinimalPlugins` 单测」
   这一条硬约束，也让地形高度变成纯函数。

### 10.5 已知问题（与代码结构相关，待决策）

- **`Armor` 没有生产者**：公式支持护甲，但组装层没给任何单位挂 `Armor`，
  目前只有测试覆盖。
- **反应窗口的粒度是"一次威胁"**：威胁持续期间玩家只被问一次（换过手就不再打断他）。
  多段攻击（连续三刀）目前只会开一次窗，将来要按"来源"分别开窗。
- **AI 不会用 Focus**：`Focus` 只挂在玩家侧，敌人声明行动时固定排前摇。
