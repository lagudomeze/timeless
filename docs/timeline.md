# 无回合时间线

> **描述对象：代码 A（仓库根 `src/`，package `app`）。**
> 本文是战斗节奏与坐标模型的权威设计，与代码同步维护。
> 配套阅读：[architecture.md](architecture.md)（模块与流水线） ·
> [components.md](components.md)（组件与系统的逐层对照）。

## 一、四条不变量

**没有回合、没有阶段、没有窗口。** 节奏与「谁说了算」由四件事决定：

| 不变量 | 载体 | 含义 |
| :--- | :--- | :--- |
| 谁能决策 | `DecisionSlot`（`Empty` / `Windup` / `Recovery`） | 行动者身上唯一的「轮到谁」判据 |
| 出手多快 | `ScheduledAction.execute_at` | 到点就执行，没有调度器替你标记状态 |
| 什么时候停 | `PauseReasons`（原因集合） | `frozen ⟺ 非空`；唯一写 `Time<Virtual>` 的是 `apply_clock` |
| 谁被威胁 | `Threatens` / `TargetCell` | 威胁由**行动自己声明**，反应系统只做读数 |

```text
[Empty] ──声明──▶ [Windup + 行动实体] ──到点──▶ 执行器落地 ──▶ [Recovery { until }]
   ▲                                                                │
   └───────── recovery_system（now >= until → DecisionReady）────────┘
```

### 状态写在决策槽里，时间戳只回答「到点了没有」

```rust
pub enum DecisionSlot {          // 行动者身上，三态直接写在这里
    Empty,                       // 空闲：可以声明行动
    Windup,                      // 前摇中：行动实体还活着，随时可以反悔
    Recovery { until: f32 },     // 后摇中：until（虚拟秒）之前不接受新决策
}

pub struct ScheduledAction {     // 行动实体身上，**只有"什么时候落地"**
    pub execute_at: f32,
}

pub struct ActionTiming {        // 也在行动实体身上：**载荷自己的节奏**
    pub windup: f32, pub recovery: f32, pub interrupt_resist: i32,
}
```

**行动者不写在调度数据里**：行动实体是行动者的**子实体**（Bevy 的 `ChildOf`），
「这条行动是谁的」由父子关系直接回答。父节点销毁时子节点跟着销毁（`Children`
是 linked spawn），因此不存在"行动者死了、行动还在半空"这种孤儿状态。

| 状态 | 判据 | 谁处理 |
| :--- | :--- | :--- |
| 前摇（可撤销 / 可打断） | `DecisionSlot::Windup`，且 `now < execute_at` | `undo_system` / `combat::formula::interrupt_observer` |
| 该执行了 | `now > execute_at` | 各领域自己的执行器（`due()`） |
| 后摇 | `DecisionSlot::Recovery { until }` 且 `now < until` | `recovery_system` |

- **`Declared` / `Pending` / `Committed` 三态被删掉**：标记与时间戳打架是这类系统的经典
  bug（"标记忘了摘，于是每帧重复触发同一个动作"）；现在动作的**阶段**是决策槽里
  唯一的枚举，**时刻**只有 `execute_at` / `until` 两个时间戳。
- **执行器自己收尾**：`if !schedule.due(now) { continue; }` → 落地效果 → 销毁行动实体 →
  `DecisionSlot::recovering(timing, now, effect_delay)` 写进行动者的决策槽。
  没有 `scheduler_system`，也没有 `begin_action` / `end_action` 这类集中式收尾函数。
- **`due()` 用严格大于**：`execute_at = now` 的零前摇行动（Focus 抢先手）因此落到
  **下一帧**执行。这一帧延迟让反应系统看得见"玩家刚举起来的那一手"，
  玩家感知上仍然是瞬时生效。
- **后摇到点只由时间线宣布**：`recovery_system` 把槽清成 `Empty` 并 trigger
  `DecisionReady`（EntityEvent，目标 = 行动者），时间线**不直接改任何资源**——
  谁关心"又轮到它决策了"（比如精力回复）谁自己订阅。

### 冻结：原因集合，而不是一个布尔

```text
冻结 ⟺ PauseReasons 非空
```

**暂停是每帧断言，不是边沿开关**：谁这一帧还想让世界停着，就写一条
`PauseRequest::Pause(原因)`；不再写，原因下一帧自然消失——不需要谁去"撤销"。

| 原因 | 谁写 | 什么时候写 |
| :--- | :--- | :--- |
| `"manual"` | `input::keyboard::pause_input_system` | 空格打开手动暂停后，**每帧重新断言**直到再按一次 |
| `"slot_empty"` | `compute_player_awaiting_system` | 场上有 `InputDriven` 单位的决策槽空着 |
| `"threat"` | `combat::reaction::detect_threat_system` | 有敌对行动 / 投射物瞄准玩家所在的格，且玩家还没表态 |

- `PauseRequest` 只有两个变体：`Pause(&'static str)`（断言）与 `Resume`（解冻，
  **不带原因**）。`process_pause_requests` 每帧先 `clear()` 再按消息顺序处理，
  `Resume` 清掉"此刻已经收集到"的原因。`PauseReasons` 内部是
  `HashSet<&'static str>`，因此"每帧断言"零分配。
- 顺序因此有意义且确定：输入域（`Resume` 的来源）排在 `TimelineSet` 之前，
  各领域的断言排在它之后——玩家手动解冻的那一帧，仍然成立的断言会照常加回来。
- **多个原因可以叠加、互不覆盖**：手动暂停不会因为"玩家刚声明了行动"而失效
  （旧实现是边沿触发的开关，空格因此只前进一帧——这是重构修掉的 bug）。
- `apply_clock` 是**唯一**写 `Time<Virtual>` 的地方；Bevy 每帧把虚拟时间拷进通用 `Time`，
  因此位移、投射物、`Lifetime`、后摇计时全部自动停表，各领域**不需要** `if paused` 分支。
- 冻结在帧末生效，因此一帧之内所有系统看到的是同一个时钟状态（不会半帧冻、半帧不冻）。

> **再也不需要「空中不冻结」这条特例**：跳跃的后摇是 0.60s，正好覆盖整条弹道，
> 落地之前决策槽一直占着（`Windup` / `Recovery`），世界自然不会停下来等输入。

## 二、两套坐标，各管一段

| | 决策层 | 结算层 |
| :--- | :--- | :--- |
| 类型 | `Cell { x, z }` | `Transform.translation`（世界单位） |
| 单位 | 格（`CELL_SIZE = 2.0` 世界单位） | 世界单位 |
| 管什么 | 谁能决策、走哪一格、锁哪一格、威胁哪几格 | 命中、射程、爆炸半径、位移 |
| 谁更新 | `move_entities_system` 只在**停下**时写 | 每帧由 `Velocity` 推进 |

- **换算只有一个入口**：`Cell::center()` 与 `Cell::from_world()`。
- **`Cell` 不每帧从 `Transform` 反推**：浮点抖动会让格子来回跳变。
- **射程以格声明、以米判定**：`AttackRange(1).world()` = 2.0 米，再与真实距离比较。
- **威胁按格声明**（反应系统），**伤害按真实距离结算**（爆炸半径、扇形夹角）。

## 三、节奏常量（各领域自己的 `*_TIMING`）

`timeline` 只提供 `ActionTiming` 这个**形状**（前摇 / 后摇 / 打断抗性），
**具体数值归载荷自己**——常量就写在载荷类型的旁边：

| 动作（载荷） | 常量 | 住哪 | 前摇 windup | 后摇 recovery | 打断抗性 |
| :--- | :--- | :--- | ---: | ---: | ---: |
| 移动 | `MOVE_TIMING` | `movement/actions.rs` | 0.15s | 0.10s | 1 |
| 跳跃 | `JUMP_TIMING` | `movement/actions.rs` | 0.10s | 0.60s | 6 |
| 翻滚 | `ROLL_TIMING` | `movement/actions.rs` | 0.05s | 0.30s | 1 |
| 近战 | `MELEE_TIMING` | `combat/skills/actions.rs` | 0.20s | 0.35s | 3 |
| 箭矢 | `ARROW_TIMING` | `combat/skills/actions.rs` | 0.30s | 0.50s | 2 |
| 火球 | `FIREBALL_TIMING` | `combat/skills/fireball.rs` | 0.30s | 0.50s | 2 |
| 招架 | `PARRY_TIMING` | `combat/defense/actions.rs` | 0.05s | 0.25s | 2 |

> 为什么不放在 `timeline/timing.rs`：那样「新增一个动作」就必须回头改时间线，
> 而时间线自己的承诺是**不感知载荷**。常量跟着载荷走之后，加动作只是
> 「载荷 + 场景工厂 + 执行器 + 声明系统」四件事，调度器一行不改。
> 声明系统要做的第一件事是 `players.iter().first_ready(&mut blocked)`（`timeline::FirstReady`）：
> 「挑出此刻能决策的行动者，挑不到就替 HUD 记下原因」只有这一份实现。
> 调度器自己的单测也因此改用自造的 `TEST_TIMING`，不再被具体载荷钉住。

> **后摇从「效果落地那一刻」起算**：执行器用
> `DecisionSlot::recovering(timing, now, effect_delay)`，其中
> `effect_delay` 允许把忙碌窗口推到效果真的发生（移动走到格中心、火球飞到落点）。
> 数值先硬编码在各领域，后续外置成 `.ron`（见 [../TODO.md](../TODO.md)）。

## 四、撤销：退款归花钱的领域

撤销由 `undo_system` 完成：**没到点**（`pending`）且**没挂 `Uncancellable`** 的玩家行动
可以被撤。撤销时**先** trigger `ActionCancelled { entity, actor }`（EntityEvent，目标 = 行动实体），
**再**销毁行动实体，并把行动者的决策槽清成 `Empty`。

```text
undo_system
  ├─ 过滤：Without<Uncancellable> + schedule.pending(now) + 行动者是 InputDriven
  ├─ trigger ActionCancelled { entity: 行动实体, actor: 行动者 }   ← 必须在 despawn 之前
  ├─ despawn 行动实体
  └─ 行动者决策槽 → Empty（行动者可能已阵亡，先 get_entity 守卫）
```

「退多少、收多少」**不住在时间线里**，而是由花钱的那个领域订阅 `ActionCancelled`
自己算——时间线根本不认识火球 / 近战这些载荷：

| 退款 Observer | 规则 |
| :--- | :--- |
| `combat::skills::fireball::refund_fireball_observer` | 退 `FIREBALL_COST`(2) 又收 2：撤销本身要有分量 |
| `combat::skills::actions::refund_melee_observer` | 收 `MELEE_CANCEL_PENALTY`(1)：抡出去再收招 |

- **翻滚 / 招架不退款**：它们的精力在**执行时**才扣（`roll_executor_system` /
  `parry_executor_system`），前摇里撤销时还没花过钱，无可退。
- **打断也不退款**：那一手白费了（`ActionCancelled` 只由 `undo_system` 触发）。
- `Uncancellable` 是**标记组件**，只回答"能不能撤"；旧的 `Cancellable` 枚举
  （`Free` / `Cost` / `Never`）已经删掉。
- 到点的行动撤不掉（来不及）；挂了 `Uncancellable` 的行动（跳跃）连前摇里也撤不掉。

## 五、打断：打的是「还没发生的事」

打断是一次**针对实体的即时响应**，因此走 `EntityEvent` + Observer，而不是 Message。
**判定的归属在战斗域**：`InterruptEvent` 与 `interrupt_observer` 都住
`combat::formula`，算式是纯函数 `interrupt_lands`（零 Bevy、可单测）。
时间线只提供它自己的两样数据——`ScheduledAction.pending()` 与
`ActionTiming.interrupt_resist`。

```rust
// combat/formula/events.rs
#[derive(EntityEvent)]
pub struct InterruptEvent { pub entity: Entity, pub source: Entity, pub power: i32 }

// 命中结算里：
commands.trigger(InterruptEvent { entity: target, source: attack, power });
```

Observer 的判定（`combat::formula::interrupt_observer`，用 `&ChildOf` 取行动者）：

```text
power == 0                                  → 直接返回（没有力度就不做对抗）
找不到该行动者 execute_at > now 的行动        → 直接返回（这一手已经出去了，打不断）
interrupt_lands(power, interrupt_resist, 3d5, 3d5)
  攻方 = power  + 3 + 3d5
  守方 = resist + 3 + 3d5
  攻方 >= 守方 → 销毁那条行动实体 + 目标决策槽清空
```

- 旧的「破势（`Impact`）」是**同刻相撞**时比大小；新模型里"谁先出手"由 `execute_at`
  决定，相撞不再需要仲裁，打断因此改成"撞掉对方还在前摇里的那一手"。
- 打断不退款：那一手白费了（`ActionCancelled` 只由撤销触发，打断不触发它）。
- AI 与玩家共用同一条规则：谁被打中前摇，谁的决策槽就被清空、下一帧重新决策。
- **为什么算式不在时间线里**：它是战斗裁决，和 `resolve_defense`（挡没挡下）、
  `counter_damage`（回敬多少）同类，因此都住 `combat/formula/domain.rs`；
  时间线只负责"这一手还占着槽"这件事本身。

## 六、反应系统：威胁 → 冻结 → 玩家表态

```text
每个 action 自己声明威胁覆盖的格（Threatens）
飞行中的投射物声明瞄准的格（TargetCell）
  ─▶ detect_threat_system：敌对来源且覆盖玩家所在格 → 每帧断言 Pause("threat")
  ─▶ 世界冻结：玩家可以撤销、换手、或者花 1 点 Focus 抢先手
  ─▶ 玩家换了一手（= 表态）→ 停止断言，原因下一帧自然消失、世界解冻
```

- **「敌对」是必要条件**：玩家自己的火球砸在自己脚下不该把世界冻住（那样球永远飞不出去）。
- **一次威胁只开一个窗口**：窗口打开时记下"玩家当时那一手"，玩家换了行动就说明他表态了；
  表态之后即使威胁还在也不再断言（否则世界会走一帧停一帧）。
- 威胁消失（前摇结束 / 被打断 / 投射物落地）后窗口复位，下一次威胁重新开窗。
- 判据用**格**而不是实体：火球声明飞行经过的格（`trajectory_cells`），
  近战声明正前方 + 左右各一格（`melee_arc_cells`），因此"站哪一格"就是全部信息。
  新增攻击方式只要挂 `Threatens`，反应系统一行不改。
- `detect_threat_system` 与时间线一样**只断言**（写 `PauseRequest`），不自己解冻：
  真正把原因落成时钟的仍然是 `process_pause_requests` → `apply_clock`。

### Focus：把前摇买掉

```rust
pub struct Focus { pub current: u32, pub max: u32 }   // 默认 3/3
const FOCUS_RECOVER_INTERVAL: f32 = 10.0;             // 每 10 虚拟秒回 1 点
```

- 玩家声明行动时按住 `Shift`：`FocusIntent` 为真 → 扣 1 点 →
  `execute_at = now`（下一帧落地，语义上不算前摇）。
- **恢复走 `Time<Virtual>`**：冻结时一分都不回，因此威胁窗口不是白送资源的时间。
- 没余量时静默退回普通前摇（不会扣成负数，也不会偷偷瞬发）。

## 七、移动

```rust
pub struct MoveAction { pub from_cell: Cell, pub to_cell: Cell }
```

```text
MoveCommand{axis} / MoveToCommand{cell}
  → declare_*_system      决策槽空着才接受 → step_from_axis 吸附成正交格步 → 行动实体
  → move_action_executor  now > execute_at → 朝格中心设 Velocity + MoveGoal
                          收尾：销毁行动实体 + Recovery 到「真的走到位」
  → move_entities_system  按速度位移；到格中心吸附 + 停 Velocity
                          + 写 Cell + 移除 MoveGoal + 兑现 DodgingOnArrival
```

- 方向键是**按一次走一格**（`input` 只在方向变化时发消息）；鼠标点地板可以跨多格。
- 斜向输入取绝对值大的分量；分量相等时固定走 Z，保证同一输入永远推出同一格。
- 一次决策的落点恒等于「相邻格的中心」；跳跃是原地弹道（`Jumping`），竖直与水平互不干扰。

## 八、防御：翻滚与招架

| 动作 | 消耗 | 效果 | 标记 |
| :--- | ---: | :--- | :--- |
| 翻滚 | 1 精力 | 远离最近威胁退一格 | `Dodging { expires_at }`（0.5s 无敌帧） |
| 招架 | 1 精力 | 挡下**绑定的那次攻击**并反制一半伤害 | `Parrying { target_attack, expires_at }` |

- 翻滚的载荷 `RollAction` 住在 `movement`，声明与落地在 `combat/defense`。
- **无敌帧必须和位移同时生效**：执行器挂 `DodgingOnArrival`，由 `move_entities_system`
  在吸附到位那一刻兑现成 `Dodging`。
- 招架找不到威胁（没有挂在自己身上的 `CollisionTarget`）就**不消耗精力、不占决策槽**。
- AI 的 `Intent::Dodge` 直接生成自己的 roll 行动，不借玩家的 `RollCommand`。

## 九、伤害：一条没有中间态的链

```text
目标获取（detect_collisions / detect_melee）→ CollisionTarget
  → apply_physical_hits_system   防御判定 + 护甲 + 打断触发 + 命中计数
  → DamageEvent                  已经算完减免的伤害（纯整数、可交换）
  → apply_damage_system          唯一扣血点：current -= amount、首次归零发 DeathEvent
  → despawn_dead_system          帧末：Health ≤ 0 的实体销毁
```

**没有两阶段裁决**：伤害是纯减法（可交换），谁先谁后不影响结果，因此不需要
`Arbitration` 快照。三段链也压成了一段（`ModifyHealthEvent` / `request_damage_system`
被删掉）。

**伤害类型不是枚举**：一种伤害 = 一个组件（`PhysicalDamage`，将来 `FireDamage`…）
+ 一个把它变成 `DamageEvent` 的系统 + 在场景工厂里挂上。生命值、死亡、日志、撤销
都不需要知道新类型存在。

**死亡只报一次**：`apply_damage_system` 比较"扣之前活着、扣之后死了"，
同一帧的多段伤害也只会有一条 `DeathEvent`（带 `killer`，供复盘）。

## 十、火球：锁格 + 真实距离 AoE

| | 箭矢（`arrow.rs`，暂未接输入） | 火球（`fireball.rs`） |
| :--- | :--- | :--- |
| 命中方式 | 碰撞（`CollisionTarget` + `HitRadius`） | 到达目标格 → 半径内全体 |
| 落点 | 追踪最近敌人 | **声明时锁定的格**（敌人可以走开） |
| 判定距离 | 碰撞半径之和 | 真实距离 ≤ `FIREBALL_RADIUS`（3.0 米） |

```text
FireCommand
  → declare_fireball_system  锁格 + 扣 2 精力 + 声明威胁覆盖的格（飞行路径）→ 行动实体
  → 前摇 0.30s
  → fireball_action_executor_system  从**当前站位**发射投射物（TargetCell + Fireball）
                                     收尾：Recovery 到 executed_at + flight_time(..)
  → projectile_arrival_system  每帧比「格中心 ↔ 投射物位置」→ ProjectileArrived
  → explosion_system           按**真实距离**取半径内敌对单位 → DamageEvent
```

- 火球行动**声明时不生成投射物、执行时才生成**：声明与落地之间还能撤销。
- 「忙到落地」是硬要求：只忙后摇的话，玩家一空闲世界就冻住，球会停在半空。

## 十一、精力与技能菜单

- `Stamina { current, max }` 是**防御与机动的货币**（翻滚 1 / 招架 1 / 火球 2），
  每次重新可决策时回 1 点：`recovery_system` trigger `DecisionReady` →
  `combat::defense::recover_stamina_observer` 落地。时间线不反向依赖 `Stamina`。
- 注册表 `combat/skills/registry.rs` 的 `SKILLS` 是**展示与消耗的单一来源**。
- 菜单选择**随时可做**（忙的时候也能先把下一个选好），释放要求决策槽为空。
- 菜单不生成行动实体、不扣资源：扣费只在各领域的声明系统里发生。

## 十二、AI 意图循环

```text
decide_intent_system（只读：距离 / 血量 / 威胁 → Intent）
enemy_declare_system（把 Intent 翻成行动实体）
```

优先级（**威胁优先于贪刀**）：

1. 有还没到点的行动把**我脚下的格**写进了 `Threatens` → `Dodge`（精力不足时降级为 `Approach`）
2. 血少（≤ `cautious_health_ratio`）且贴脸 → `Retreat`
3. 超出 `engage_range` → `Approach`
4. 贴脸（≤ `MELEE_REACH`）→ `Melee`
5. 在武器射程内（`AttackRange::world()`）→ `Shoot`（火球锁住目标**当前**那一格）
6. 其余 → `Approach`

玩家与 AI 看的是**同一张威胁图**：都用 `Threatens` 覆盖的格判定。没有独立的冷却系统，
「多久能再决策」由上一个动作的后摇决定。

## 十三、扩展点

| 想加的东西 | 落点 | 需要改调度器吗 |
| :--- | :--- | :--- |
| 新动作（冲刺、陷阱、召唤） | 新载荷 + 场景工厂 + 执行器 + 它自己的 `*_TIMING`（写在载荷旁边） | 不用 |
| 新攻击方式 | 新目标获取系统（挂 `CollisionTarget`）+ 复用伤害链 +（**记得**挂 `Threatens`） | 不用 |
| 新元素伤害 | 新组件 + 一个同形的命中系统 + 挂在工厂上 | 不用 |
| 新的暂停原因 | `timeline::resources` 加常量 + 一个 `compute_*` 系统 | 不用 |
| 新消耗资源（弹药 / 架势） | 新组件 + 消费 `ActionCancelled` 的系统 | 不用 |
| 多敌人 / 新怪物 | `spawn` 加一组零件；AI 把「最近」换成威胁排序 | 不用 |
| 实时压力模式 | 后摇改用 `Time<Real>` 基准，其余不动 | 不用 |

> **冻结期间投射物与后摇一起停表**是刻意的：世界是「等玩家想好」，不是「实时压力」。
