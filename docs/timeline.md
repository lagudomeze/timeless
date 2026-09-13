# 无回合时间线

> **描述对象：代码 A（仓库根 `src/`，package `app`）。**
> 本文是战斗节奏与坐标模型的权威设计，与代码同步维护。
> 配套阅读：[architecture.md](architecture.md)（模块与流水线） ·
> [components.md](components.md)（组件与系统的逐层对照）。

## 一、三条不变量

**没有回合、没有阶段、没有全局状态机。** 节奏只由两件事决定：

| 不变量 | 载体 | 含义 |
| :--- | :--- | :--- |
| 谁能决策 | `Ready` 组件 | 这是无回合模型里**唯一的「轮到谁」判据** |
| 出手多快 | `ActionTiming { windup, recovery }` | 每个动作自带前摇 + 后摇，没有冷却计时器 |
| 什么时候停 | `timeline_gate_system` | **全局唯一的暂停点**，写 `Time<Virtual>` |

```text
       玩家 Ready ⟹ 冻结 Time<Virtual>（世界真的停下来等）
输入 ─▶ 声明动作（移除 Ready）⟹ 虚拟时间恢复流动
       ─▶ 前摇到点 → 执行器落地 → 后摇（BusyRecovery）
       ─▶ 后摇结束 ⟹ 恢复 Ready（+1 精力）⟹ 又轮到它
```

敌人不等玩家：它一有 `Ready` 就自己决策（`ai::decide_intent_system` →
`enemy_declare_system` 写与玩家**同一条**消息）。玩家与 AI 因此共用同一套
声明 → 调度 → 执行链，防御与技能都只有一份实现。

### 冻结的判据

`timeline_gate_system` 是**唯一**按游戏状态写 `Time<Virtual>` 的地方：

```text
冻结 ⟺ 场上存在玩家 且 没有单位在空中 且（玩家 Ready 或 反应窗口判定有威胁）
```

- **为什么「空中不冻结」**：跳跃是不可中断的弹道；若玩家落地前恢复 `Ready` 就停表，
  单位会僵在半空。等它落地再等输入。
- **为什么各领域没有 `if paused`**：Bevy 每帧把虚拟时间拷进通用 `Time`，
  所以位移、投射物、`Lifetime`、后摇计时**自动**停表。
- **反悔不用「确认」**：声明即生效（`commit_bridge_system` 当帧升 `Pending`），
  改主意走打断 / 撤销——右键，或直接按下一个新意图。

## 二、两套坐标，各管一段

最容易搞混的一点：**决策在格子上，结算在真实空间里**。

| | 决策层 | 结算层 |
| :--- | :--- | :--- |
| 类型 | `Cell { x: i32, z: i32 }` | `Transform.translation`（世界单位） |
| 单位 | 格（`CELL_SIZE = 2.0` 世界单位） | 世界单位 |
| 管什么 | 谁能决策、走哪一格、锁哪一格、同格判定 | 命中、射程、爆炸半径、位移 |
| 谁更新 | `move_entities_system` 只在**停下**时写 | 每帧由 `Velocity` 推进 |
| 落点 | `movement/cell.rs` | Bevy `Transform` |

- **换算只有一个入口**：`Cell::center()`（格心，`(x + 0.5) * CELL_SIZE`）与
  `Cell::from_world(Vec3)`（`floor` 回推）。格的**角**才是 `x * CELL_SIZE`。
- **`Cell` 不每帧从 `Transform` 反推**：浮点抖动会让格子来回跳变，
  决策层必须只在「吸附到位」那一刻更新。
- **为什么 `CELL_SIZE = 2.0`**：体素是 1×1×1，若一格等于一个体素，
  单位会被压成 1 米大小，与模型缩放和视觉尺度对不上；2.0 让「一格 = 两步体素」。
- **射程以格声明、以米判定**：`AttackRange(1)`（`AttackRange::MELEE`）经
  `.world()` 换算成 `1 × CELL_SIZE = 2.0` 米，再与真实距离比较。
- **同格判定**用 `Cell` 比较（踩格、占地、AI 的「是否已贴脸」）；
  **不用它算伤害**。

## 三、行动实体与状态机

行动 = **独立实体**。实体上只有调度数据与状态标记，调度器**永远不读载荷**：

```rust
pub struct ScheduledAction {
    pub actor: Entity,
    pub timing: ActionTiming,
    pub declared_at: f32,   // 虚拟秒
    pub execute_at: f32,    // declared_at + timing.windup
}
```

```text
   声明 ─▶ Declared ──(commit_bridge_system，当帧)──▶ Pending ──(到点)──▶ Committed
                        │                                              │
                        └── 可撤销（右键 / 新意图打断）                   └── 执行器落地
                                                                            │
                              Ready ◀──(后摇走完，+1 精力)── BusyRecovery ◀──┘
```

- `Declared` 只活一帧：声明系统当帧挂上，提交桥当帧或下一帧摘掉。
- `Committed` 表示「**本帧**等待执行器处理」。执行器收尾**必须**走
  `timeline::end_action`，否则 `With<Committed>` 的执行器每帧重复触发同一个动作
  （历史 bug：跳跃无限上升、技能连发）。
- `Ready` 在声明时移除、后摇结束时恢复；**忙的时候不接受新声明**（事件被静默丢弃 +
  一条 `ActionBlocked` 让 HUD 说清原因）。
- 隔离缓冲用 `Arbitration` **资源**，不用 `Local`：两个结算阶段是两个系统，
  各自的 `Local` 不共享（历史 bug：阶段 2 静默什么都不做）。

### 收尾纪律：忙到「效果真的发生」

`end_action_until(.., busy_until)` 可以把行动者的忙碌窗口推到**效果落地**那一刻。
带位移 / 飞行的动作**必须**用它，否则玩家一恢复 `Ready`，门控立刻冻结虚拟时间，
效果会被冻在半路（实机表现：按了技能没放出去，但精力已经扣了）。

| 动作 | `busy_until` |
| :--- | :--- |
| 移动 / 翻滚 | 走到目标格所需的 `距离 / 速度` |
| 火球 | `executed_at + flight_time(origin, target_cell)` |
| 跳跃 | 后摇本身（0.60s）覆盖整条弹道 |
| 近战 / 招架 | 只用后摇 |

## 四、节奏常量（`timeline/timing.rs`）

| 动作 | 前摇 windup | 后摇 recovery | 说明 |
| :--- | ---: | ---: | :--- |
| 移动 `MoveAction` | 0.15s | 0.10s | 一格一步 |
| 跳跃 `JumpAction` | 0.10s | 0.60s | 后摇覆盖整条弹道，落地即可再决策 |
| 近战 `MeleeAction` | 0.20s | 0.35s | 出手快、硬直长 |
| 火球 `FireballAction` | 0.30s | 0.50s | 出手慢、威力大 |
| 翻滚 `RollAction` | 0.05s | 0.30s | 防御性，几乎立即生效 |
| 招架 `ParryAction` | 0.05s | 0.25s | 同上 |

> **后摇从「落地时刻」起算**，不是从「执行时刻」：`end_action` 用
> `executed_at + timing.recovery`。这样即便后摇为 0，恢复系统也不会在同一帧
> 就把 `Ready` 加回来。

数值先集中硬编码，后续外置成 `.ron`（见 [../TODO.md](../TODO.md)）。

## 五、反应窗口与打断

`F2` 循环 `TimelineConfig.reaction` 三档，管的是「**敌人打过来时要不要停下来等玩家**」：

| 档位 | 行为 |
| :--- | :--- |
| `Loose`（默认） | 只要场上有「正在前摇、且瞄准玩家」的攻击就冻结（最松，方便调试） |
| `Strict` | 只在玩家**能反应**（就绪且不在空中）时冻结 |
| `Off` | 完全不因威胁冻结 |

威胁的判据是**那条未结算的行动正瞄着玩家**（挂着 `CollisionTarget(玩家)`）——
用现成的标记而不是自建威胁表，因此箭矢、横扫、未来的任何攻击方式自动算威胁。

`interrupt_system` 负责「改主意」：本帧只要出现**玩家直接产生的意图**
（`MoveCommand` / `MoveToCommand` / `JumpCommand` / `RollCommand` / `ParryCommand` /
`UseSelectedSkill`），就把玩家那条**可取消的**未结算行动撤掉，后面的声明系统照旧接手。

它刻意**不监听** `FireCommand` / `MeleeCommand`：这两条是 `UseSelectedSkill` 派生的
下游消息，晚一帧才出现，监听会把「刚声明出来的火球」当成新意图撤掉。

取消的代价挂在**行动实体自己**身上，不是全局规则：

| 组件 | 语义 |
| :--- | :--- |
| `ActionCost(n)` | 声明时花了多少 → 撤销时原样退还 |
| `CancelCost(n)` | 撤它要付多少（**没挂 = 免费**） |
| `Uncancellable` | 根本不给撤（跳跃：前摇里也撤不掉） |

火球 `CancelCost(2)`、近战 `CancelCost(1)`、移动与翻滚免费——
「赶路调整方向」不该收费，「大招打断」才该。

## 六、移动

```rust
pub struct MoveAction { pub from_cell: Cell, pub to_cell: Cell }
```

```text
MoveCommand{axis} / MoveToCommand{cell}
  → declare_*_system      step_from_axis 吸附成正交格步 → 行动实体（Declared）
  → commit_bridge_system  Declared → Pending
  → scheduler_system      时间到 → Committed
  → move_action_executor_system
         Velocity = 朝 to_cell.center() 的方向 × MoveSpeed
         insert MoveGoal{cell: to_cell}，busy_until = 距离 / 速度
  → move_entities_system  按速度位移；到格中心吸附 + 停 Velocity
                          + 写 Cell + 移除 MoveGoal + 兑现 DodgingOnArrival
  → end_action_until      摘 Committed + 销毁行动实体 + 挂 BusyRecovery
```

- 方向键是**按一次走一格**：`input` 只在方向**变化**时发消息（`Local` 记住上次方向）。
  按住不会连走，因此不会顶掉刚按下的技能键。
- 鼠标点地板可以**跨多格**：同一条载荷 + 同一条执行器，忙多久按距离自动变长。
- 斜向输入取绝对值大的分量；分量相等时固定走 Z，保证同一输入永远推出同一格。
- **一次决策的落点恒等于「相邻格的中心」**；起点可能在格角上，所以世界位移向量
  不一定是正交的——不变式是「终点 = 相邻格中心」，不是「位移正交」。
- 跳跃是原地弹道（`Jumping`，`v += g·dt`，落回起跳高度即结束），
  竖直位移与水平移动互不干扰。

## 七、防御：翻滚与招架

两个**反应性动作** + 两个短命标记：

| 动作 | 消耗 | 效果 | 标记 |
| :--- | ---: | :--- | :--- |
| 翻滚 | 1 精力 | 远离最近威胁退一格 | `Dodging { expires_at }`（0.5s 无敌帧） |
| 招架 | 1 精力 | 挡下**绑定的那次**攻击并反制一半伤害 | `Parrying { target_attack, expires_at }` |

- 翻滚的载荷 `RollAction` 住在 `movement`（位移是移动的原语），
  声明与落地在 `combat/defense`。
- **无敌帧必须和位移同时生效**：提前挂会在原地就无敌，推迟挂会在飞出去之后留破绽。
  因此执行器挂的是 `DodgingOnArrival`，由 `move_entities_system` 在吸附到位那一刻
  兑现成 `Dodging`。
- 招架找不到威胁（没有正在前摇的攻击）就**不消耗精力、不占用这次决策**——
  招架是反应，不该因为「空气招架」白掉一次行动机会。
- AI 的 `Intent::Dodge` **直接生成自己的 roll 行动**，不借玩家的 `RollCommand`
  （借用会让玩家的 `E` 键把就绪的敌人一起带着滚）。两者产出的行动实体完全一样，
  区别只在触发源。

## 八、火球：锁格 + 真实距离 AoE

两种投射物，两套命中机制：

| | 箭矢（`arrow.rs`，暂未接输入） | 火球（`fireball.rs`） |
| :--- | :--- | :--- |
| 命中方式 | 碰撞（`CollisionTarget` + `HitRadius`） | 到达目标格 → 半径内全体 |
| 落点 | 追踪最近敌人 | **声明时锁定的格**（敌人可以走开） |
| 判定距离 | 碰撞半径之和 | 真实距离 ≤ `FIREBALL_RADIUS`（3.0 米，1.5 格） |

```text
FireCommand
  → declare_fireball_system  锁「最近敌对单位所在的格」或鼠标点的那一格
                             扣 2 精力 + 记 ActionCost(2) → FireballAction（Declared）
  → …前摇 0.30s…             fireball_action_executor_system 从**当前站位**发射投射物
                             busy_until = executed_at + flight_time(..)
  → projectile_arrival_system 每帧比「格中心 ↔ 投射物位置」，≤ 0.2 即到
                             → ProjectileArrived{origin, damage, radius, faction}
  → explosion_system         按**真实距离**取半径内敌对单位 → DamageEvent
                             → 无论打中打空都销毁投射物
```

**「决策按格、结算按真实距离」在这里最直观**：点的是格，炸的是米；
站在同一格的边缘和格中心会真的吃到不同结果。落点在**声明那一刻**定死，
敌人之后走开就炸空——这正是「预判」的博弈点。

> 火球行动**声明时不生成投射物、执行时才生成**：声明与落地之间的这段还能被撤销，
> 声明时就生成会留下撤不干净的半空火球。

## 九、两阶段结算与三层裁决

```text
phase1_arbitrate_system  只读：两遍扫描
  ① 把本帧所有攻击的真实参数收成快照 [(攻击, 目标, 真实距离, AttackStats)]
  ② 逐条做防御判定 + 三层裁决 → CombatResult
phase2_apply_system      落地：唯一扣血路径
  DamageEvent / 招架反制 / 命中计数 / 清 CollisionTarget
```

**为什么要快照**：同刻互击时，两边必须看到**同一份**参数，否则「互杀」是否成立
取决于系统调用顺序。阶段 1 不改任何组件，阶段 2 才统一落地。

**三层裁决**（`formula/domain.rs`，纯函数）：

```text
L1  AttackFrame   帧小者先
L2  真实距离      AttackRange::world() 够得着的先
L3  Impact        破势大者打断对方（伤害归零，结论改写为 Interrupted）
```

`Side` 是**相对裁决调用者**的视角，因此同刻互击的两次调用会各自镜像一次。
「帧」在无回合模型里不是物理时钟，而是这一层的排序权重：`execute_at` 决定谁先出手，
三层裁决决定**同刻相撞**时谁占优，两者正交。

**扣血流向是一条三段链**（别写成一段）：

```text
phase2_apply_system → DamageEvent → health::request_damage_system
                    → ModifyHealthEvent → apply_damage（唯一扣血入口）
```

`DamageEvent` 同时被战斗日志读；治疗 / 中毒 / 再生直接写 `ModifyHealthEvent` 即可。

## 十、精力与技能菜单

- `Stamina { current, max }` 是**防御与机动的货币**（翻滚 1 / 招架 1 / 火球 2），
  每次恢复 `Ready`（后摇结束）回 1 点。没有「每回合 +1」——无回合没有回合。
- 注册表 `combat/skills/registry.rs` 的 `SKILLS` 是**展示与消耗的单一来源**：
  菜单、HUD、可用性判断都读它，「加一个技能」只需加一条 `SkillDef`。
- 菜单 `MenuSelection` **选择随时可做**（忙的时候也能先把下一个选好），
  **释放要求当前就绪**；循环只在当前负担得起的技能之间走。
- `SkillKind::Attack` 按**真实距离**派发：贴脸（≤ `MELEE_REACH` = 3/4 格）→ 近战，
  否则 → 火球。键盘 `G`、数字键、鼠标左键点单位走的是同一条路径。
- 菜单**不生成行动实体、不扣精力**：扣费只在各领域的声明系统里发生。

## 十一、AI 意图循环

「选意图」与「声明行动」拆成两个系统，因此 HUD 能在敌人动手**之前**读到它想干什么：

```text
decide_intent_system（只读：距离 / 血量 / 威胁 → Intent）
enemy_declare_system（把 Intent 翻成行动实体）
```

优先级（**威胁优先于贪刀**）：

1. 有攻击正在前摇、且把我当目标 → `Dodge`（精力不足时降级为 `Approach`）
2. 血少（≤ `cautious_health_ratio`）且贴脸 → `Retreat`
3. 超出 `engage_range` → `Approach`
4. 贴脸（≤ `MELEE_REACH`）→ `Melee`
5. 在武器射程内（`AttackRange::world()`）→ `Shoot`（火球锁住目标**当前**那一格）
6. 其余 → `Approach`

没有独立的冷却系统：敌人「多久能再决策」由它上一个动作的后摇决定。
`EnemyBrain` 用 `#[require(Intent)]` 声明依赖——少了 `Intent` 两个 AI 系统会
**静默地一行都不执行**（敌人站着不动），用 `require` 让组装层不可能再漏。

单目标（最近敌人）；多敌人的扩展点是把「最近」换成各自的威胁排序。

## 十二、扩展点

| 想加的东西 | 落点 | 需要改调度器吗 |
| :--- | :--- | :--- |
| 新动作（冲刺、陷阱、召唤） | 新载荷组件 + 工厂 + 执行器 +（可选）`ActionTiming` 常量 | 不用 |
| 新攻击方式（穿透、AOE） | 新目标获取系统（挂 `CollisionTarget`）+ 复用扣血链 | 不用 |
| 新元素伤害 | `formula` 加变体与一个纯函数；目标获取 / 生命 / 清理都不动 | 不用 |
| 新消耗资源（弹药 / 架势） | 新组件 + 一个消费 `ActionCancelled` 的系统 | 不用 |
| 多敌人 / 新怪物 | `spawn` 加一组零件；AI 把「最近」换成威胁排序 | 不用 |
| 实时压力模式 | 把后摇记在 `Time<Real>` 上，其余不动 | 不用 |

> **冻结期间投射物与后摇一起停表**是刻意的：世界是「等玩家想好」，
> 不是「实时压力」。要做真实时模式，只改后摇的时钟基准。
