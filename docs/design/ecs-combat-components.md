# ECS 战斗组件化设计（代码 A）

> **范围**：仓库根目录 `src/`（package `app`）的战斗 / 移动领域组件建模。
> 这是**当前实现**的参考，与代码同步维护。
>
> 配套阅读：[无回合时间线](timeline-turnless.md)（权威设计） ·
> [app 领域模块](app-modules.md) · [Bevy 0.19 速查](../bevy/bevy-019.md)。
>
> 本文替代 2026-09 之前的同名文档：那一版描述的是已废弃的 We-Go 版本
> （`Phase` / `RoundEnded` / `Can*` 能力标记 / `Position` + `GridMath` 网格坐标）。

## 一、设计原则

1. **实体 = 小组件的组合**。每个可独立变化、可独立判定的维度拆成一个组件；
   没有「单位属性包」聚合结构。
2. **成对出现**：攻击方带什么，命中时就按什么结算。`Health` ↔ `PhysicalDamage`、
   `AttackFrame` ↔ `AttackFrame`（同刻互击比帧）、`Parrying` ↔ 被绑定的那次攻击。
3. **行动即实体**：没有行动枚举、没有行动队列。`MoveAction` / `JumpAction` /
   `RollAction` / `MeleeAction` / `ShootAction` / `FireballAction` / `ParryAction`
   是**载荷组件**，配合 `ScheduledAction` + `Declared → Pending → Committed`
   由时间线调度，执行器收尾时 despawn。
4. **调度器不感知载荷**：`timeline/` 只读 `ScheduledAction` 与状态标记；
   新增动作（冲刺、陷阱、召唤…）只新增载荷与执行器，不改调度器。
5. **决策按格、结算按真实距离**（本文第四节）。
6. **领域层零 Bevy**：`combat/formula/domain.rs` 是纯函数 + 纯数据，
   裁决参数在调用处组装成 `AttackStats` 传进去，可脱离 ECS 单测。
7. **两阶段结算**：阶段 1 只读裁决（同一份快照），阶段 2 统一落地——
   消除「谁先被系统调用谁占便宜」的先手偏差（本文第七节）。
8. **实体构建用 BSN**：`bsn!` + `spawn_scene`；组件派生 `Default + Clone`，
   含 `Entity` 字段的手写 `Default`（`Entity::PLACEHOLDER`）。

## 二、坐标：两套坐标，各管一段

这是本设计里最容易搞错的一点，先单独说清楚。

| | 决策层 | 结算层 |
| :--- | :--- | :--- |
| 类型 | `Cell { x: i32, z: i32 }` | `Transform.translation`（世界米） |
| 单位 | 格（`CELL_SIZE = 2.0` 世界单位） | 世界单位 |
| 管什么 | 谁能决策、走哪一格、锁哪一格 | 命中、射程、爆炸半径、位移 |
| 谁更新 | `move_entities_system` 只在**停下**时写 | 每帧由 `Velocity` 推进 |
| 位置 | `movement/cell.rs` | Bevy `Transform` |

关键推论：

- **`Cell` 不每帧从 `Transform` 反推**。浮点抖动会让格子来回跳变，
  决策层必须只在「吸附到位」那一刻更新。
- **格中心是 `Cell::center()`**（`(x + 0.5) * CELL_SIZE`），格的**角**是
  `x * CELL_SIZE`。`unit_scene` 直接把地形采样点当出生位置，因此单位初始
  可能站在**格角**上——第一次移动会顺便把它带到格中心。
- **射程以格声明、以米判定**：`AttackRange(1)`（`AttackRange::MELEE`）
  经 `.world()` 换算成 `1 × CELL_SIZE = 2.0` 米，再和真实距离比。
  近战贴脸判据 `MELEE_REACH = CELL_SIZE * 0.75 = 1.5` 米（
  `combat/skills/menu.rs` 与 `ai/systems.rs` 共用同一约定）。
- **输入是屏幕方向，格步是正交的**：`input::GroundBasis` 把 `WASD` 按相机朝向
  换算到世界 XZ 平面，`movement::step_from_axis` 再吸附成**一格的正交步**
  （斜向输入取绝对值大的分量；完全相等时定走 Z，保证同一输入永远推出同一格）。

## 三、组件清单

### 3.1 调度与状态（`timeline/`）

| 组件 | 字段 | 语义 | 生命周期 |
| :--- | :--- | :--- | :--- |
| `ScheduledAction` | `actor` / `timing` / `declared_at` / `execute_at` | 动作实体调度数据；`execute_at = declared_at + timing.windup` | 动作实体 |
| `Declared` | —（ZST） | 草案：`require_commit = true` 时等 `Enter` | 动作实体 |
| `Pending` | —（ZST） | 已提交、等到点 | 动作实体 |
| `Committed` | —（ZST） | **本帧**到点、等待执行器处理 | 动作实体 |
| `Ready` | —（ZST） | **唯一的「轮到谁」判据**；声明动作时移除，后摇结束时恢复 | 单位 |
| `BusyRecovery` | `executed_at` / `ready_at` | 后摇窗口：`ready_at` 之前不接受新决策 | 单位 |

> `Committed` **必须**由执行器通过 `timeline::end_action` 摘掉并销毁动作实体。
> 忘了它，同一个动作会被所有 `With<Committed>` 的执行器每帧重复触发
> （历史 bug：跳跃无限上升、技能连发）。

### 3.2 移动与格子（`movement/`）

| 组件 | 字段 | 语义 |
| :--- | :--- | :--- |
| `Cell` | `x` / `z` | 单位当前所在格（决策层坐标） |
| `MoveGoal` | `cell` | 走到这一格就吸附停下；由执行器挂上、落地时移除 |
| `Velocity` | `Vec3`（世界单位/秒） | 位置每帧 `+= 速度 × dt`；单位、投射物通用 |
| `MoveSpeed` | `f32`（格/秒） | 单位移动速度，用来算 `Velocity` |
| `MoveAction` | `from_cell` / `to_cell` | 移动载荷：走一格 |
| `JumpAction` | —（ZST） | 跳跃载荷 |
| `Jumping` | `ground_y` / `velocity` | 跳跃弹道状态；落回 `ground_y` 即移除 |
| `RollAction` | `from_cell` / `to_cell` | 翻滚载荷（退一格）；**住在移动领域**，因为位移是移动原语 |
| `DodgingOnArrival` | `expires_at` | 「到位那一刻挂无敌帧」的请求（见 3.4） |

### 3.3 战斗基础（`combat/`）

| 组件 | 字段 | 语义 |
| :--- | :--- | :--- |
| `Faction` | `Player` / `Enemy` | 参战阵营：目标过滤、友伤、AI 选敌共用一条查询路径 |
| `Collidable` | —（ZST） | 参与碰撞检测的纯物理过滤标记 |
| `Health` | `current` / `max` | 生命值（`combat/health`） |
| `PhysicalDamage` | `f32` | 单次攻击伤害（输出侧） |
| `Armor` | `f32` | 物理减伤（防御侧） |
| `HitRadius` | `f32` | 命中半径；距离 ≤ 两者半径之和即接触 |
| `AttackRange` | `u32`（**格**） | 射程；判定前 `.world()` 换算成米。`MELEE` = 1 格、`RANGED` = 2 格 |
| `AttackFrame` | `u32` | 速度帧，**越小越先命中**（三层裁决 L1） |
| `Impact` | `u32` | 破势，同刻互击时打断对方（三层裁决 L3） |
| `Stamina` | `current` / `max` | 精力：翻滚 / 招架 / 火球的货币；重新 `Ready` 时 +1 |

数值一览（`combat/skills/` 与 `timeline/timing.rs`）：

| 技能 | 伤害 | 帧 | 破势 | 精力 | 前摇 | 后摇 |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: |
| 近战横扫 | 15 | 5 | 3 | 0 | 0.20 | 0.35 |
| 箭矢（未接输入） | 10 | 4 | 1 | 0 | 0.30 | 0.50 |
| 火球 | 12 | 7 | 2 | 2 | 0.30 | 0.50 |
| 翻滚 | 0 | — | — | 1 | 0.05 | 0.30 |
| 招架 | 0 | — | — | 1 | 0.05 | 0.25 |
| 移动 | — | — | — | 0 | 0.15 | 0.10 |
| 跳跃 | — | — | — | 0 | 0.10 | 0.60 |

### 3.4 防御（`combat/defense/`）

| 组件 | 字段 | 语义 | 生命周期 |
| :--- | :--- | :--- | :--- |
| `ParryAction` | `target_attack: Entity` | 招架载荷：挡下**指定的那次攻击** | 动作实体 |
| `Dodging` | `expires_at`（虚拟秒） | 翻滚后的无敌帧标记，到期移除 | 闪避窗口 |
| `Parrying` | `target_attack` / `expires_at` | 招架标记：绑定被挡的攻击，并带兜底过期时间 | 本次结算前 |

> **为什么翻滚要 `DodgingOnArrival`**：无敌帧必须和位移**同时**生效。
> 提前挂会在原地就无敌，推迟挂会在飞出去之后留破绽。因此它跟着 `MoveGoal` 走，
> 由 `move_entities_system` 在吸附到位那一刻兑现成 `Dodging`。

### 3.5 攻击实体形状与生命周期

| 组件 | 字段 | 语义 |
| :--- | :--- | :--- |
| `Projectile` | `max_hits` / `current_hits` / `finished` | 射弹逻辑状态，自包含穿透计数 |
| `HitOnce` | `spent` | 一次性命中开关（近战横扫命中一次后不再结算） |
| `Lifetime` | `Timer` | 攻击实体存活时长（近战横扫默认 0.18s，到期自动销毁） |
| `MeleeShape` | `range` / `half_arc` | 近战扇形：以自身为圆心、朝 `Transform` 正前方扫过 |
| `CollisionTarget` | `Entity` | **临时**目标标记：目标获取每帧挂上、阶段 2 消费后移除 |

### 3.6 技能载荷（`combat/skills/`）

| 组件 | 字段 | 语义 |
| :--- | :--- | :--- |
| `MeleeAction` | —（ZST） | 近战载荷：到点生成一次横扫 |
| `ShootAction` | —（ZST） | 射击载荷：到点生成一支箭（**当前未被玩家输入触发**） |
| `FireballAction` | —（ZST） | 火球载荷：到点只收尾进入后摇（投射物在**声明时**就已生成） |
| `Fireball` | `target_cell` / `speed` / `amount` / `radius` | 火球**投射物**：锁定的格 + 飞行与爆炸参数 |

### 3.7 AI（`ai/`）

| 组件 | 字段 | 语义 |
| :--- | :--- | :--- |
| `EnemyBrain` | `engage_range` / `cautious_health_ratio` | 决策参数（射程不在这里——那是武器属性） |
| `Intent` | `Idle` / `Approach` / `Melee` / `Shoot` / `Retreat` / `Dodge` | 本次决策选出的意图（HUD / 复盘读它） |

### 3.8 资源

| 资源 | 字段 | 语义 |
| :--- | :--- | :--- |
| `Timeline` | `waiting_for_input` / `draft` | 唯一的暂停真相 + 玩家未提交的草案 |
| `TimelineConfig` | `require_commit` | `false`（默认）= 输入直接生效；`true` = 等 `Enter` |
| `Arbitration` | `results` / `snapshot` | **两阶段结算的隔离缓冲**（见第七节） |
| `MenuSelection` | `index` | 当前选中的技能下标 |

## 四、双层坐标在工作流里的体现

### 4.1 移动：声明一格 → 连续位移 → 吸附

```text
MoveCommand{axis}
  → declare_move_system      step_from_axis 吸附成正交格步 → MoveAction{from,to}（Declared）
  → commit_bridge_system     Declared → Pending
  → scheduler_system         Time<Virtual> 到点 → Committed
  → move_action_executor_system
        Velocity = 朝 to_cell.center() 的方向 × MoveSpeed
        insert MoveGoal{cell: to_cell}
  → move_entities_system     按速度位移；到格中心则吸附 + velocity=0
                             + 写 Cell + 移除 MoveGoal + 兑现 DodgingOnArrival
  → end_action               摘 Committed + 销毁动作实体 + 挂 BusyRecovery
```

一次决策的落点恒等于「**相邻格的中心**」。注意起点可能在格角上，
所以世界位移向量不一定是正交的——不变式是「终点 = 相邻格中心」，不是「位移正交」。

### 4.2 火球：声明时锁格 → 自由飞行 → 到达后按米结算

```text
FireCommand
  → declare_fireball_system  锁「最近敌对单位所在的那一格」→ FireballAction（Declared）
                             同时立刻 spawn 火球投射物（Velocity 朝格中心）
  → …到点…                   fireball_action_executor_system 只收尾进后摇
  → projectile_arrival_system 每帧比「格中心 ↔ 投射物位置」，≤ ARRIVAL_TOLERANCE(0.2) 即到
                              → 广播 ProjectileArrived{origin, damage, radius, faction}
                              → 摘 Fireball + 标记 Projectile{finished}
  → explosion_system         按**真实距离**取半径内敌对单位 → DamageEvent
                              → despawn 投射物（打空也要销毁）
```

「锁格」的语义正在这里：落点在**声明那一刻**定死，敌人之后走开就炸空了
（`fireball_whiffs_on_empty_ground` 就是这条不变式的可执行证明）。

## 五、消息清单（谁写 → 谁消费）

输入类消息全部由 `input/` 翻译产生，消费方在自己领域注册 `add_message::<T>()`。

| 消息 | 写 | 消费 |
| :--- | :--- | :--- |
| `MoveCommand { axis }` | `input` | `movement::declare_move_system` |
| `JumpCommand` | `input` | `movement::declare_jump_system` |
| `FireCommand` | `input` / 菜单派发 | `skills::declare_fireball_system` |
| `MeleeCommand` | `input` / 菜单派发 | `skills::declare_melee_system` |
| `RollCommand` | `input` / 菜单派发 / `ai::enemy_declare_system` | `defense::declare_roll_system` |
| `ParryCommand` | `input` | `defense::declare_parry_system` |
| `SelectSkill(usize)` / `CycleSkill { forward }` | `input` | `skills::menu`（只改 `MenuSelection`） |
| `UseSelectedSkill` | `input` | `menu::use_selected_skill_system`（派发成上面几条） |
| `ActionsCommitted` | `input`（`Enter`） | `timeline::commit_bridge_system` |
| `ResetBattle` | `input`（`R`） | `spawn::reset_battle_system` |
| `PanCamera { delta }` | `input` | `presentation::camera_pan_system` |
| `ProjectileArrived { … }` | `skills::projectile_arrival_system` | `skills::explosion_system` |
| `DamageEvent { target, amount, kind }` | 阶段 2 / `explosion_system` | `health::request_damage_system` |
| `ModifyHealthEvent { target, amount }` | `request_damage_system` | `apply_damage` |
| `DeathEvent { entity }` | `apply_damage` | `despawn_dead_system`、战斗日志 |
| `AttackResolved { attacker, target, outcome, counter }` | 阶段 2 | 战斗日志 / HUD |
| `ChunkLoadEvent` / `ChunkUnloadEvent` / `ChunkDirtyEvent` | `world` | `voxel_render` |

**链式分工**：`DamageEvent`（已算完护甲）→ `ModifyHealthEvent`（血量增减请求）
→ `DeathEvent`（归零）。治疗 / 中毒 / 再生复用同一条后段，不必碰前段。

## 六、动作实体状态机

```text
                      require_commit = false（默认）
  声明 ──▶ Declared ──────────────────────────▶ Pending ──▶ Committed ──▶ 执行器 ──▶ despawn
              │                                   ▲                                  │
              └── require_commit = true ──────────┘                                  │
                  等 ActionsCommitted（Enter）                                       │
                                                                                     ▼
                                                        BusyRecovery{ready_at} ──▶ Ready（+1 精力）
```

- **同一单位同一时刻至多一个动作**：约束来自 `Ready`——声明即移除 `Ready`，
  忙的时候声明系统根本匹配不到这个单位，输入因此不会排队到下一次决策。
- **`Ready` 是唯一的「轮到谁」判据**，取代了旧模型的 `Phase::Planning`。
- **后摇结束同时是最自然的回精力点**：`recovery_system` 挪 `BusyRecovery`
  并 `Stamina::regen(1)`。无回合模型没有「回合」，这是 +1 的落点。

## 七、两阶段结算与 `Arbitration`

```text
phase1_arbitrate_system   只读：两遍扫描
  ① 把本帧所有攻击的真实参数收成快照 [(攻击, 目标, 真实距离, AttackStats)]
  ② 逐条做防御判定 + 三层裁决 → CombatResult
phase2_apply_system       落地：唯一扣血路径
  DamageEvent / 招架反制 / 命中计数 / 清 CollisionTarget
```

**为什么要快照**：同刻互击时，两边必须看到**同一份**参数，否则「互杀」是否成立
取决于系统调用顺序。阶段 1 不改任何组件，阶段 2 才统一落地。

**为什么隔离缓冲是 `Resource` 而不是 `Local`**：`Local<T>` 是**每系统独享**的，
两个阶段各自初始化会拿到两份不同的缓冲——阶段 1 写的那份没人读
（历史 bug：阶段 2 静默什么都不做）。`Arbitration` 资源让「谁写谁读」在类型上一目了然。

**三层裁决**（`formula/domain.rs`，纯函数）：`AttackFrame`（帧小者先）
→ 真实距离（够得着的先）→ `Impact`（破势大者打断对方）。
`Side` 是**相对裁决调用者**的视角，因此同刻互击的两次调用会各自镜像一次。

## 八、火球为什么不是特例分支

火球没有专属系统分支，它只是「组件组合 + 一条消息」：

```rust
// skills::fireball_scene —— 略去视觉部分
bsn! {
    template_value(faction)
    template_value(Velocity(direction * FIREBALL_SPEED))   // 通用位移组件
    template_value(Fireball { target_cell, speed, amount, radius })
    Projectile { max_hits: 0, current_hits: 0, finished: false }
    template_value(PhysicalDamage(FIREBALL_DAMAGE))
    template_value(AttackFrame(7))
    template_value(Impact(2))
    HitRadius(0.35)
    Transform { translation: {origin}, rotation: {rotation} }
    // …Mesh3d / MeshMaterial3d
}
```

`move_entities_system` 不认识火球，只按 `Velocity` 推进（带 `MoveGoal` 的才吸附）；
`projectile_arrival_system` 只读 `Fireball.target_cell`；
`explosion_system` 只读 `ProjectileArrived` 与 `Health`。
**没有任何系统知道「火球」这个概念**，所以换一个锁定方式（比如锁单位而不是锁格）
只需要换一套载荷 + 一条消息。

## 九、结算流水线（实际系统链）

跨领域顺序只在 `lib.rs::configure_pipeline` 里声明一次
（测试复用同一入口，因此跑的就是真实流水线）：

```text
Update:  SpawnSet ─▶ InputSet ─▶ TimelineSet ─▶ AiSet ─▶ MovementSet ─▶ CombatSet
         ─▶ VoxelRenderSet ─▶ PresentationSet

TimelineSet:  timeline_gate_system（唯一暂停点）→ commit_bridge_system
              → scheduler_system → recovery_system

CombatSet:    expire_defense_markers_system          // 本帧到期的无敌帧不该再生效
              → (select_skill_system, cycle_skill_system) → use_selected_skill_system
              → (declare_fireball, declare_melee, declare_roll, declare_parry)
              → (melee_executor, fireball_executor, roll_executor, parry_executor)
              → (projectile_arrival_system, explosion_system)   // 本帧到达本帧结算
              → detect_collisions_system → detect_melee_system
              → phase1_arbitrate_system → phase2_apply_system  // 两阶段
              → request_damage_system → apply_damage → despawn_dead_system
              → cleanup_finished_attacks_system → expire_attack_entities_system
```

**唯一的暂停点**是 `timeline_gate_system`：

```text
冻结 Time<Virtual> ⟺ 场上存在玩家 且 玩家 Ready 且 没有单位在空中
```

因此各领域**没有任何 `if paused` 分支**——Bevy 每帧把虚拟时间拷进通用 `Time`，
位移、投射物、`Lifetime`、后摇计时全部自动停表。
「空中不冻结」是必要的：否则玩家落地前恢复 `Ready` 会让单位僵在半空。

## 十、领域文件归属

```text
timeline/     components(调度 + 状态标记) · resources(Timeline / TimelineConfig)
              timing(ActionTiming + 常量表) · events(ActionsCommitted)
              systems(门控 / 提交桥 / 调度 / 后摇 + end_action / begin_action)
movement/     cell(Cell / MoveGoal) · components(Velocity / MoveSpeed)
              actions(MoveAction / JumpAction / RollAction + 声明 / 执行器)
              systems(move_entities_system + DodgingOnArrival) · events(MoveCommand / JumpCommand)
combat/
  components.rs   Faction / Collidable
  attributes/     PhysicalDamage / Armor / HitRadius / AttackRange / AttackFrame / Impact
  health/         Health + ModifyHealthEvent / DeathEvent + apply_damage / despawn_dead
  targeting/      CollisionTarget / MeleeShape + 碰撞与扇形检测
  lifecycle/      Projectile / HitOnce / Lifetime + 清理系统
  formula/        domain(零 Bevy 裁决) · events(DamageEvent) · resolution(两阶段 + Arbitration)
  defense/        stamina · components(Dodging / Parrying / ParryAction / AttackResolved)
                  actions(翻滚 / 招架的声明与执行) · systems(标记过期)
  skills/         events(FireCommand / MeleeCommand) · registry(SKILLS / SkillKind)
                  menu(MenuSelection + 选择 / 循环 / 派发) · melee · arrow · fireball · explosion
                  actions(ShootAction / MeleeAction；箭矢暂未接输入)
ai/           EnemyBrain / Intent + decide_intent_system + enemy_declare_system
input/        keyboard(按键 → 消息) + pointer(中键 → PanCamera)
presentation/ 相机 / 装饰 / 战斗日志 / HUD（只读）
spawn/        组装车间：unit(共用零件) / player / enemy / assembly / restart
```

## 十一、从旧版迁移的对照

| 旧版（已删除） | 现在 |
| :--- | :--- |
| `Phase` / `RoundEnded` / 1s 推进窗口 | `Ready` + 每动作自带的 `ActionTiming`；唯一暂停点 |
| `TurnCommitted` / `phase_advance_system` | `ActionsCommitted` / `commit_bridge_system` |
| `Position` + `GridMath`（`IVec2`） | `Transform` + `Cell`（两套坐标明确分工） |
| `Destination`（投射物目标格） | `MoveGoal{cell}`；火球用 `Fireball.target_cell` |
| `LinearVelocity`（格/秒） | `Velocity`（世界单位/秒） |
| `ExplosionDamage` 组件 | `Fireball.amount` / `.radius` + `ProjectileArrived` 消息 |
| `MoveTo` / `Roll` / `Attack` / `Parry` 载荷 | `MoveAction` / `RollAction` / `MeleeAction` `ShootAction` / `ParryAction` |
| `Damage` 组件 | `PhysicalDamage`（输出侧）+ `Armor`（防御侧） |
| `CombatResult` 作为组件挂在攻击上 | `Arbitration` 资源里的普通 struct，阶段 2 drain |
| `CanAttack` / `CanMove` / `CanRoll` / `CanFireball` | 删掉：`Ready` + `SKILLS` 注册表 + `MenuSelection` |
| `AttackCooldown` | 删掉：后摇（`BusyRecovery`）就是冷却 |
| `finalize_declared_actions` / `commit_system` | `commit_bridge_system` |
| `reaction_execution_system`（实时反应） | 翻滚 / 招架与其它动作走**同一条**声明 → 调度 → 执行链 |
| `stop_on_round_end_system` | 删掉：没有「轮」可结束 |
| `death_check_system` / `message_log_system` | `despawn_dead_system` / `presentation::battle_log_system` |
| `spawn_fireball`（执行器里生成） | `declare_fireball_at`（**声明时**生成，因此落点不会被敌人跑掉改追） |

## 十二、社区参考

- **bevy-compose**（`Health(i32)` + `Damage(i32)`）：验证了「属性成对拆分」可行。
- **maciejglowka 的 bevy_turn_based**（2024）：行动实体化 + `hit → damage → kill`
  事件链；本项目据此把「算伤害」与「扣血」拆成两段消息。
- **endless docs / projectiles**：投射物 movement 与 collision 分离——
  对应 `Velocity`（通用位移）与 `targeting`（碰撞判定）的分离。
- **Bevy 官方 two-phase / deferred command 讨论**：本项目用自己的 `Arbitration`
  资源而非 `Local` 缓冲来表达「同帧强顺序的一对系统」。
