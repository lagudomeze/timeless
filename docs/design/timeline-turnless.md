# 无回合时间线设计（决策按格 · 结算按真实距离）

> 描述对象：**代码 A（仓库根 `src/`，package `app`）** — 本文是 A 的权威时间线设计。
> 状态：**已拍板并开始落地**（2026-09 会话）。
> 取代：[timeline.md](timeline.md)（逻辑刻度 + We-Go 阶段机）、
> [timeline-core-design.md](timeline-core-design.md)（v0.1 接口签名）。
> 关系：能力来自**代码 B**（`timeless/`）的迁移；坐标与渲染沿用 A 的世界空间体素原型。
> 冲突分析见 [../status.md](../status.md)。

---

## 0. 三条已定决策（本设计的约束）

| 决策 | 内容 | 影响 |
| :--- | :--- | :--- |
| **D1** | 保留 **A** 为主线树，B 的能力迁进来 | 目录/分层/BSN/测试基座不动；B 的战斗能力重写为 A 的领域化形态 |
| **D2** | **无回合**：所有 PC/NPC「能决策就决策」；仅当**玩家等待输入**时冻结虚拟时间；默认输入**直接生效**，另有 `require_commit` 开关 | 删除 `Phase` / `round` / `RESOLUTION_WINDOW` / `RoundEnded` / `pause_during_planning_system` |
| **D3** | 保留 A 的**真实距离**结算；**决策**与同格判定按**格子** | 单位/行动落在格上，命中/射程/爆炸用世界距离 |
| **D4** | 节奏 = **固定冷却**：每个动作自带**前摇 + 后摇** | 动作表决定出手快慢；速度属性暂不引入 |
| **D5** | 投射物**锁定目标格** → 自由飞行 → 到达后按真实距离结算 AoE | 弹道为实体运动（A 已有），落点检定与 AOE 半径用世界距离 |

> ⚠️ 旧阶段机（`Phase` / `Timeline::window` / 提交窗口）**彻底删除**。
> 旧形态在本文件第 7 节的映射表与 `../status.md` 第六节里留档；
> 仓库是 git 仓库（`main` 分支），迁移过程另有提交记录可查。

---

## 1. 两条正交的坐标轴（本设计的核心）

最容易搞混的一点：**决策在格子上，结算在真实空间里**。两者用格子边长换算。

```text
决策层（格子，整数）                      结算层（世界，连续）
─────────────────────────                ─────────────────────────
Cell { x, z }  = 单位在哪一格             Transform.translation: Vec3
step_from_axis / 同格判定 / 逼近 / 射程格数  距离 / 碰撞半径 / AoE 半径 / 弹道飞行
```

- **格子边长** `CELL_SIZE = 2.0`（世界单位/格，定义在 `timeline/timing.rs`）。
  选 2.0 而不是 1.0 的理由：体素是 1×1×1，若 `CELL_SIZE = 1.0`，
  格子与体素一一对应会把单位压成 1 米大小，与现有模型缩放和视觉尺度对不上；
  2.0 让「一格 = 两步体素」，单位模型可正常缩放。
- **换算只有一个入口**：`Cell::center() -> Vec2`（格中心）与
  `Cell::from_world(Vec3) -> Cell`（世界坐标回推，`floor`）。
  注意 `Cell::center()` 是 `(x + 0.5) * CELL_SIZE`——**格心**，不是格的角
  （`x * CELL_SIZE`）；`unit_scene` 用地形采样点出生，因此起点可能在格角上。
- **两个坐标必须同步**：`Cell` 由 `move_entities_system` 在「吸附到目标格中心」
  那一刻更新，不每帧从 `Transform` 反推（避免浮点抖动导致格子跳变）。

**射程怎么写**：`AttackRange(pub u32)`（**格数**）在结算时换算成世界距离
`AttackRange::world() = self.0 as f32 * CELL_SIZE`，命中判定仍是「真实距离 ≤ range_world」。
这样设计稿里的「射程 1 格」和几何命中能同时成立。常量：`MELEE = 1` 格、`RANGED = 2` 格。

**同格判定**（「谁和谁在同一格」）用 `Cell` 比较，用于：踩格、占地、格子级威胁提示、
AI 的「是否已在攻击格」判断。**不用它算伤害。**

---

## 2. 动作生命周期：为什么不需要 Phase

一个单位在任意时刻只有两种状态：**能决策（`Ready`）** 与 **忙（没有 `Ready`）**。
「什么时候结算」「什么时候轮到我」全部由每个动作自己的时间决定，不再有全局阶段。

```text
[Ready] ──声明动作──▶ [Busy] ──到点执行──▶ [Recovery] ──后摇走完──▶ [Ready]
   │                    │                      │
   │                    │                      └─ 期间不接受新声明
   │                    └─ execute_at = 声明时刻 + 前摇（FixedTiming.windup）
   └─ 玩家在 Ready 且等待输入时，时间被冻结（唯一的暂停点）
```

### 数据结构

```rust
/// 动作的固定节奏：前摇 + 后摇。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActionTiming {
    pub windup: f32,       // 声明 → 执行
    pub recovery: f32,     // 执行 → 重新 Ready
}

/// 行动实体的调度数据（调度器只读它，不认识载荷）。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ScheduledAction {
    pub actor: Entity,
    pub timing: ActionTiming,
    pub declared_at: f32,  // 虚拟秒
    pub execute_at: f32,   // declared_at + timing.windup
}

/// 状态标记：草案（可选，见第 3 节）→ 待执行 → 已到点。
#[derive(Component)] pub struct Declared;
#[derive(Component)] pub struct Pending;
#[derive(Component)] pub struct Committed;

/// 单位「现在可以决策」。声明动作时移除，后摇结束时恢复。
#[derive(Component, Debug, Default, Clone, Copy)] pub struct Ready;

/// 后摇窗口：`ready_at` 之前不接受新决策。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct BusyRecovery { pub executed_at: f32, pub ready_at: f32 }
```

> `ActionTiming` 只有两个字段。旧稿曾写第三个字段 `arrive_at`（前缀冗余、便于 HUD），
> **实际不采用**：HUD 要「什么时候能再决策」直接读 `BusyRecovery::ready_at`
> （它用的是**落地时刻 + 后摇**，而不是执行时刻 + 后摇）。

### 系统链（`TimelineSet` 内，顺序即语义）

```text
timeline_gate_system      每帧算：是否有单位 Ready 且等待玩家输入 → pause/unpause Time<Virtual>
commit_bridge_system      require_commit = false 时把 Declared 直接升为 Pending；true 时等 ActionsCommitted
scheduler_system          Pending 且 now >= execute_at → Committed
（各领域执行器）            Committed → 落地效果（设速度 / 挂防御标记 / 生成投射物）→ despawn 行动实体
recovery_system           执行器落地时给 actor 挂 BusyRecovery{until}；到期恢复 Ready
```

去掉的东西：`Timeline::phase` / `round` / `window` / `RESOLUTION_WINDOW` / `begin_resolution` /
`tick_resolution` / `end_round` / `end_round_system` / `RoundEnded` / `pause_during_planning_system`。

> `stop_on_round_end_system` 也删掉：移动不再靠「窗口结束」停下，
> 而是**走到目标格就停**（第 5 节）。这反而修掉了旧模型「窗口一过单位突然刹住」的毛病。

> **分层影响**：`Ready` 是「单位能决策」的零件，因此 `spawn` 现在也依赖 `timeline`
> （`spawn ──▶ ... ──▶ timeline`）。组装层贴共用零件本来就是它的职责，
> 反向依赖（任何领域依赖 `spawn`）依然禁止。见 `src/spawn/mod.rs` 的依赖图。

---

## 3. 玩家输入门控与 `require_commit` 开关

### 3.1 谁在等谁

```rust
#[derive(Resource, Debug)]
pub struct Timeline {
    /// 玩家已 Ready 但还没动作：世界停下等他。
    waiting_for_input: bool,
    /// 玩家本轮的草案（require_commit = true 时才有值）。
    draft: Option<Entity>,
}
```

- `waiting_for_input = (存在玩家实体) && (玩家 Ready) && (没有单位在空中)`。
  最后一条不能省：跳跃是不可中断的弹道，若玩家落地前恢复 `Ready` 就停表，单位会僵在半空。
- `timeline_gate_system`：`waiting_for_input == true` → `Time<Virtual>::pause()`，否则 `unpause()`。
  这是**全局唯一的暂停点**，各领域依旧不需要任何 `if paused` 分支——
  移动、计时器、后摇、投射物生命周期全部自动停表。

> **语义说明（重要）**：冻结期间 NPC 的后摇也一并不走，**飞行中的投射物也一起停表**。
> 这是刻意的：世界是「等玩家想好」而不是「实时压力」。若将来要做真实时压力模式，
> 只需把后摇记在 `Time<Real>` 上，其余不动——这一点作为扩展点明确留出。
>
> 副作用（写测试时会撞上）：玩家一恢复 `Ready` 时间就冻结，因此**一发远程火球会停在半空**，
> 直到玩家再次做决策。`fireball_whiffs_on_empty_ground` 因此必须每帧喂输入让时间继续走。

### 3.2 默认：输入直接产生效果

`require_commit = false`（默认）时：

```text
按 W        → MoveCommand → declare_move_system → 挂 ScheduledAction（windup 0.15）→ 0.15s 后起步
按 E（近战） → MeleeCommand → 立即执行
按 Q（火球） → FireCommand → 立即朝目标格飞行
```

没有「先声明再确认」这一步；玩家按下就是决定。声明完成后 `Ready` 被移除；
按住方向键**不会**继续走（见 3.4），但**松开再按**会在后摇结束后走出下一格。

### 3.3 开关：`require_commit = true`

```text
按 W        → 只生成草案（Declared），时间仍然冻结
按 Enter    → ActionsCommitted → 草案升为 Pending → 时间恢复流动
```

- 草案同一时刻至多一条：`begin_action` 把新草案记进 `Timeline::draft`，
  旧的 `Declared` 实体**不会**被主动清掉，而是因为玩家已失去 `Ready`
  而不再接受新声明（「后声明覆盖先声明」由输入不排队保证）。
- HUD 在草案存在时提示 `WAITING FOR INPUT  (draft ready, Enter to commit)`。
- 实现位置：`commit_bridge_system` 读一个 `TimelineConfig` 资源，**不散落到各领域**。

```rust
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimelineConfig {
    /// true = 需要 Enter 确认；false（默认）= 输入立即生效。
    pub require_commit: bool,
}
```

开关本身只能由 `F1` 切换（`commit_mode_toggle_system`），没有面板。

> 兼容旧行为：把 `require_commit` 设为 true，玩家的手感就与旧 `Planning` 阶段几乎一致，
> 区别只是 NPC 不再等玩家（它们按自己的节奏走）。

### 3.4 按住键：一次决策一格

`input` 只在**方向变化**时发一次 `MoveCommand`（`Local` 记住上次方向），
`declare_*_system` 也只处理有 `Ready` 的单位。于是：

- **按一次 W = 走一格**（决策按格，不需要连发消息）；
- 按住 W 不会每帧重复声明，因此**不会顶掉**玩家刚按下的技能键；
- 想连续走两格：松开再按一次（一格一次决策）。

> 为什么不像旧模型那样每帧写消息：无回合模型里「按住」是一种持续状态，
> 而决策是离散事件；把两者混在一起会让技能键被移动键瞬间覆盖。
> 这是格子决策模型的直接推论。
>
> **对比**：§3.2 的「按住不会继续走」指的就是这条；`player_skill_input_system`
> 里的技能键用的是 `just_pressed`，两者在同一次决策里不会互相覆盖。

### 3.5 冷却 / 忙时的输入

忙的时候收到的输入按现有约定**丢弃**（不回放），并在 HUD 上给出反馈
（`PLAYER ... busy`）。旧写法「推进阶段不接受新声明」改为「没有 `Ready` 不接受新声明」，
即把 `timeline.is_planning()` 全部替换为 `Ready` 查询——这是迁移中最机械、
也最容易漏的一步（见第 8 节清单）。

---

## 4. 节奏参数（`Timing` 常量表）

固定冷却模型的全部数值集中在一处（`timeline/timing.rs`），先硬编码，
Phase 2.1 再外置成 `.ron`：

| 动作 | 前摇 windup | 后摇 recovery | 说明 |
| :--- | ---: | ---: | :--- |
| 移动 `MoveAction` | 0.15s | 0.10s | 一格一步 |
| 跳跃 `JumpAction` | 0.10s | 0.60s | 后摇覆盖整个弹道（约 0.6s），落地即可再决策 |
| 近战 `MeleeAction` | 0.20s | 0.35s | 出手快、硬直长 |
| 火球 `FireballAction` | 0.30s | 0.50s | 出手慢、威力大 |
| 翻滚 `RollAction` | 0.05s | 0.30s | 防御性，几乎立即生效 |
| 招架 `ParryAction` | 0.05s | 0.25s | 同上 |

> **后摇从「落地时刻」起算**，不是从「执行时刻」起算：
> `end_action` 用 `executed_at + timing.recovery`。
> 这样即便后摇为 0，恢复系统也不会在同一帧就把 `Ready` 加回来。
> 跳跃的后摇因此必须覆盖整条弹道，否则玩家会在半空恢复 `Ready`。

数值验收方式：一次交锋内「我打两下、敌人打一下」这类节奏由这些常量决定，
后续调参只改这张表（配 `Command`/`SKILLS` 消耗）。

---

## 5. 移动：格为目标，连续插值

```rust
/// 移动载荷：从哪一格走到哪一格（目标格在声明时锁定）。
#[derive(Component)] pub struct MoveAction { pub from_cell: Cell, pub to_cell: Cell }

/// 单位当前所在格（决策层坐标）。
#[derive(Component)] pub struct Cell { pub x: i32, pub z: i32 }
```

- **声明**：`from_cell = *Cell`；`to_cell = from_cell + step_from_axis(axis)`，
  `step_from_axis` 把任意平面方向吸附成**正交的一格**（`(±1,0)` / `(0,±1)`）：
  斜向输入取绝对值大的分量，分量相等时固定走 Z。决策永远是「走一格」，不存在半格。
- **执行**：给行动者 `Velocity = ground_direction(to_cell.center() - translation.xz()) * move_speed`
  与 `MoveGoal { cell: to_cell }`。
- **停下**：`move_entities_system` 本帧位移不超过剩余距离；
  到格中心（`≤ 1e-3`）时 `translation.xz = cell.center()`、`Velocity = 0`、
  `Cell = to_cell`、移除 `MoveGoal`。于是「走到哪停哪」，不依赖任何窗口。
- **跳跃**：保留 `Jumping` 弹道（`v += g·dt`），落回起跳高度即移除该组件；
  竖直位移与水平移动互不干扰（水平仍由 `MoveGoal` 管）。

---

## 6. B 的能力迁移：逐项落点

### 6.1 精力（`Stamina`）

```rust
#[derive(Component)] pub struct Stamina { pub current: u32, pub max: u32 }
/// 恢复到 Ready 时回复的精力（取代 B 的「每轮回 1」）。
pub const STAMINA_REGEN_PER_DECISION: u32 = 1;
```

归属：`combat/defense/stamina.rs`（和 `Dodging` / `Parrying` 同域——精力是**防御与机动的货币**，
不是通用的「属性」）。
回复时机：`recovery_system` 恢复 `Ready` 的那一次结算里 +1（上限 `max`）；
不再有「回合结束」事件可挂，这也是删 `RoundEnded` 的连带影响之一。

> 观测注意：`try_spend` 发生在**执行那帧**，而回复发生在后摇结束时（例如
> `ROLL.recovery = 0.30`）。测试若在两者之间读 `Stamina`，会看到「扣了又回了」。

### 6.2 翻滚（`Roll` + `Dodging`）— **已落地**

- 载荷 `RollAction { from_cell, to_cell }` 住在 [`crate::movement`]（位移是移动的原语），
  声明与落地在 `combat/defense/actions.rs`。
- 声明：`F` 键 → 远离最近敌对单位的方向 → `step_from_axis` 吸附成一格 → 1 精力。
- 执行：`Velocity` 指向目标格中心 → 复用第 5 节的到格逻辑 → 同时挂
  `Dodging { expires_at: now + DODGE_SECS }`（`DODGE_SECS = 0.5`，对齐 B 的 `DODGE_MS = 500`）。
- 清理：`expire_defense_markers_system` 到点移除。

### 6.3 招架（`Parry` + `Parrying`）— **已落地**

- 载荷 `ParryAction { target_attack }`：指向**威胁的来源实体**——即「正在前摇指向我的那次
  行动的发起者」。`declare_parry_system` 用 `Query<&ScheduledAction, With<Declared>>`
  取第一条草案，把 `schedule.actor` 记进 `target_attack`；判定时
  `resolve_defense` 拿攻击实体的 `actor` 与之比对。
- 执行：给行动者挂 `Parrying { target_attack, expires_at }`（`PARRY_SECS = 0.5` 兜底），
  比 B 只绑实体更稳——目标攻击被销毁后标记不会悬空。
- 判定：`resolve_defense` 纯函数决定 `Landed / Dodged / Parried`；
  招架 = 免伤 + 反制 `counter_damage(incoming) = ceil(incoming / 2)`（至少 1）。
  **反制伤害也要落地到 `apply_damage`**，不在防御系统里直接改血量。
- 空挥保护：没有威胁时**不消耗精力、不占用这次决策**，避免「空气招架」白掉一回合。

### 6.4 火球（`Fireball` + 投射物 + 爆炸）— **已落地**

- 载荷 `FireballAction` 只负责前摇节奏（`timing::SHOOT`：0.30 / 0.50），
  **投射物在声明时就一起生成**（前摇结束不是「发射」，而是可以再决策）。
- 落点在**声明时锁定**：`target_cell = Cell::from_world(最近敌人的位置)`。
  敌人若在飞行期间走开就会躲掉——这正是「预判」的博弈点。
- 飞行：`fireball_scene` 生成投射物（`Velocity` 朝格中心、`Fireball{target_cell, speed, amount, radius}`），
  由 `projectile_arrival_system` 判定「到格」：真实距离 ≤ `ARRIVAL_TOLERANCE`（0.2 世界单位）
  → 吸附到格中心、广播 `ProjectileArrived{ origin, faction, damage, radius }`。
- 爆炸：`explosion_system` 用**真实距离**筛出半径 `FIREBALL_RADIUS = 3.0`（1.5 格）内的
  敌对 `Health` 实体 → `DamageEvent`；空地上爆炸**什么也不发生**（不留残留实体）。
  纯函数 `radial_damage_units` 单独单测「谁被炸到」。
- 数值：`FIREBALL_DAMAGE = 12`、`FIREBALL_SPEED = 8.0`（世界单位/秒）、
  玩家消耗 `FIREBALL_COST = 2` 精力（比翻滚贵，构成资源取舍）。
- 与箭矢的分工：箭矢是**单体碰撞**投射物（追踪最近敌人），火球是**锁格 AoE**；
  两者共用 `Projectile` / `Velocity` / `apply_damage`，不共用命中判定。

> 落点半径用世界距离（D3/D5），所以「站格子边缘 vs 格中心」会真实影响是否吃到爆炸——
> 这正是「决策按格、结算按真实距离」的手感来源。

### 6.5 两阶段结算 — **已落地**

```text
phase1_arbitrate_system（读 Arbitration 资源）
    只读裁决：三层裁决 + 防御判定 → 最终伤害（快照一致，谁先算谁不占便宜）
phase2_apply_system（写 Arbitration 资源）
    统一落地：DamageEvent / 招架反制 / 命中计数 + finished / 清 CollisionTarget
        ↓
    request_damage_system → apply_damage（**唯一**扣血入口）
```

- 纯逻辑放 `combat/formula/domain.rs`（**零 Bevy 依赖**，可脱离 App 单测，11 个单测）：
  `AttackStats { frame, range, impact, damage }` · `resolve_attack` · `resolve_combat` ·
  `HitOrder` / `Side` / `HitResult` · `DefenseState` + `resolve_defense` + `counter_damage`。
- **三层裁决按 A 的坐标模型改写**：B 的第二层是「格距离」，A 是「真实距离」——
  L1 `AttackFrame` 小者先 → L2 `AttackRange::world()` 大者先 → L3 `Impact` 大者打断小者。
- 配置：近战 帧 5 / 破势 3；箭矢 帧 4 / 破势 1；火球 帧 7 / 破势 2。
- 隔离缓冲用 **`Arbitration` 资源**，不用 `Local` 也不用消息。
  `Local<T>` 是**每系统独享**的，两个阶段各自初始化会拿到两份缓冲、谁也读不到谁
  （这正是实现期间踩过的坑）；而项目的 `Event` + `Observer` 约定只服务于
  「即时、定向实体」的响应。
- **同刻互击**：`phase1` 先做一遍**参数快照**（`(攻击实体, 目标, 真实距离) → AttackStats`），
  两边看到的是同一份数据，各自的裁决互为镜像（A→B 的 `Side::Attacker` 即 B→A 的 `Side::Defender`）。
  被破势打断的一方伤害归零、`DefenseOutcome` 改写为 `Interrupted`（日志要留下这次交锋）。
- **扣血流向是一条三段链**（别写成一段）：
  `phase2_apply_system` → `DamageEvent` → `health::request_damage_system` →
  `ModifyHealthEvent` → `apply_damage`。
  其中 `DamageEvent` 同时被 `presentation::battle_log_system` 读（日志），
  `apply_damage` 消费的是 `ModifyHealthEvent`。治疗 / 中毒 / 再生直接写
  `ModifyHealthEvent` 即可，不必经过 `DamageEvent`。
- 「帧」在无回合模型里不是物理时钟，而是这一层的排序权重；`execute_at` 决定**谁先出手**，
  三层裁决决定**同刻相撞时谁占优**，两者正交。

### 6.6 技能菜单与资源 HUD — **已落地**

- 注册表 `combat/skills/registry.rs`：`SkillKind` / `SkillDef` / [`SKILLS`] 一张表
  （种类 · 标签 · 精力消耗 · 节奏 · 威力），菜单、HUD、可用性判断都读它，
  因此「加一个技能」只加一条。数值与载荷共用常量（`MELEE_DAMAGE` / `FIREBALL_DAMAGE`…）。
- 菜单 `combat/skills/menu.rs`：`MenuSelection`（资源）+ `SelectSkill` / `CycleSkill` /
  `UseSelectedSkill` 三条消息 + 三个系统。**选择随时可做**（忙的时候也能先把下一个选好），
  **释放要求当前就绪**。
- `SkillKind::Attack` 按**真实距离**派发：贴脸（≤ 3/4 格）→ 近战，否则 → 火球。
- 循环选择只在**当前负担得起**的技能之间走，玩家不会把选择停在按不出来的技能上。
- 输入（`input/keyboard.rs`，只翻译）：`1`~`4` 直选 · `Tab`/`Shift+Tab` 循环 · `G` 释放；
  `Q`/`E`/`Space`/`F`/`V` 仍是快捷施放。
- HUD：`skills >1:attack (2)  2:melee (0) …` 一行 —— `>` 标当前选择，
  负担不起的打 `x`；玩家行显示 `EN current/max`，敌人行显示意图与距离。
- **不用 bevy_egui**（B 的调试面板依赖）——A 保持依赖面干净，调试信息进 HUD。

### 6.8 键位总表（A 的当前约定）

| 键 | 动作 | 说明 |
| :--- | :--- | :--- |
| `WASD` / 方向键 | 走一格 | 方向变化时才发消息 |
| `Q` | 火球（快捷） | 等价于选中 `fireball` 再释放 |
| `E` | 近战横扫（快捷） | 前摇 0.20 / 后摇 0.35 |
| `Space` | 跳跃 | 弹道 0.6s |
| `F` | 翻滚（快捷） | 1 精力，退一格 + 0.5s 无敌帧 |
| `V` | 招架（快捷） | 1 精力，挡下绑定的那次攻击并反制 |
| `1`~`4` | 直选技能 | 对应 `SKILLS` 的顺序 |
| `Tab` / `Shift+Tab` | 循环技能 | 只在当前负担得起的技能之间走 |
| `G` | 释放选中技能 | `Attack` 按真实距离派发近战 / 火球 |
| `Enter` | 提交草案 | 仅在 `require_commit` 开启时有效 |
| `F1` | 切换 `require_commit` | 控制台打印当前模式 |
| `R` | 重置战斗 | 功能键，跟着 `spawn/restart.rs` 走 |
| 中键拖拽 | 平移相机 | 只翻译成 `PanCamera` 消息 |

### 6.7 AI 意图循环 — **已落地**

「选意图」与「声明行动」拆成两个系统，HUD 因此能在敌人动手**之前**读到它想干什么：

```text
decide_intent_system（只读：距离 / 血量 / 威胁 → Intent）
enemy_declare_system（把 Intent 翻成行动实体，防御意图翻成 RollCommand）
```

优先级（**威胁优先于贪刀**）：

1. 有攻击正在前摇、且把我当目标 → `Dodge`（精力不足时降级为 `Approach`）
2. 血少（≤ `cautious_health_ratio`）且贴脸 → `Retreat`（退一格重新评估）
3. 超出 `engage_range` → `Approach`
4. 贴脸（≤ `MELEE_REACH` = 3/4 格）→ `Melee`
5. 在武器射程内（`AttackRange::world()`）→ `Shoot`（火球锁住目标当前那一格）
6. 其余 → `Approach`

- **威胁 = 任何正在前摇、且 `CollisionTarget` 指向我的攻击**。用现成的标记而不是自建
  「威胁表」，因此箭矢 / 横扫 / 未来的任何攻击方式都自动算威胁。
- **翻滚复用玩家那条路径**：AI 只写 `RollCommand`，落到同一个 `declare_roll_system`，
  防御逻辑只有一份实现（`declare_roll_system` 也因此改成遍历所有 `Ready` 单位，
  而不是只处理玩家）。
- `Intent` 是组件，HUD 直接读它；`Idle` / `Approach` / `Melee` / `Shoot` / `Retreat` / `Dodge`
  六种意图都有对应的可读标签。
- 仍是单目标（最近敌人）；Phase 3 的多敌人只需把「最近」换成各自的威胁排序。

---

## 7. 旧文档与本设计的映射

| 旧约定 | 本设计 | 说明 |
| :--- | :--- | :--- |
| `Phase::{Planning, Resolving}` | **删除** | 换成每单位 `Ready` + 每动作 `ActionTiming` |
| `RESOLUTION_WINDOW`（1s 窗口） | **删除** | 移动走到目标格为止；其余动作各有后摇 |
| `RoundEnded` / `round()` | **删除** | 需要计步时用「决策次数」而非轮次 |
| `commit_actions_system` / `ActionsCommitted` | **保留** `ActionsCommitted`（默认旁路） | 仅 `require_commit = true` 时参与 |
| `pause_during_planning_system` | `timeline_gate_system` | 暂停条件从「规划阶段」变成「玩家等待输入」 |
| `execute_at = 提交时刻 + 前摇` | `execute_at = 声明时刻 + 前摇` | 声明即开始前摇（无回合） |
| 逻辑刻度 `hit_clock` / `GlobalTime` | `execute_at`（虚拟秒）+ 领域层破平 | 「帧」= `execute_at` 排序键，不再引入第二套时钟 |
| `PendingHit` 实体 | `Fireball.target_cell` + `projectile_arrival_system` + `ProjectileArrived` | 到达后按真实距离结算，等同物化延迟命中（**没有** `ArrivesAtCell` 这个类型） |
| `AttackRange(u32)` 切比雪夫 | `AttackRange(cells)` → `.world()` 真实距离 | 结算用真实距离（D3） |
| `MoveTo { velocity: IVec2 }` | `MoveAction { from_cell, to_cell }` + 世界插值 | 决策按格、表现连续 |
| `Position(IVec2)` | `Cell { x, z }` + `Transform` 双层坐标 | 见第 1 节 |
| `Roll` / `Parry` / `Attack` 载荷 | `RollAction` / `ParryAction` / `MeleeAction` + `ShootAction` | 载荷归各自领域 |
| `Can*` 能力标记 | `Ready` + `SKILLS` 注册表 + `MenuSelection` | 删除能力标记组件 |
| `AttackCooldown` | **删除** | 后摇（`BusyRecovery`）就是冷却 |
| `CombatResult` 作为组件 | `Arbitration` 资源里的普通 struct | 阶段 2 drain 掉，不留在实体上 |
| `CancelPrivilege` / `try_cancel` | **不引入** | 无回合下「取消」就是在自己 `Ready` 时改主意，天然无需特权组件 |

---

## 8. 迁移路线图

> **状态：M1–M7 全部完成，M8（文档收口）进行中。**
> `cargo test` = 99 通过（`src/` 下 97 + `tests/assets.rs` 2）/ 0 失败 / 0 跳过；`cargo clippy --all-targets -- -D warnings` 零警告。
>
> 下面的勾选框保留为**当时的实施清单**（验收证据是测试名），
> 与代码有细节偏差的地方已就地标注。进度总览见 [../status.md](../status.md)。

### M1 时间线改造（地基）— **完成**

- [x] `timeline/resources.rs`：删 `Phase` / `round` / `window` / `RESOLUTION_WINDOW`；
      换成 `Timeline { waiting_for_input, draft }` + `TimelineConfig { require_commit }`。
- [x] `timeline/components.rs`：`ScheduledAction` 加 `timing` / `declared_at`；新增 `Ready`、`BusyRecovery`、`ActionTiming`。
- [x] `timeline/systems.rs`：删 `pause_during_planning_system` / `end_round_system`；
      新增 `timeline_gate_system` / `commit_bridge_system` / `recovery_system`；`scheduler_system` 改用 `Timeline` 新字段。
- [x] `timeline/events.rs`：删 `RoundEnded`；`ActionsCommitted` 保留（受 `require_commit` 控制）。
- [x] `timeline/timing.rs`（新）：第 4 节的常量表。
- [x] 旧单测重写成整机用例：`input_applies_immediately_by_default`、
      `require_commit_defers_execution_until_enter`、`recovery_restores_the_ability_to_decide`。
- [x] 全局替换：所有 `timeline.is_planning()` → 「actor 有 `Ready`」查询。

### M2 双层坐标与格子移动 — **完成**

- [x] `movement/cell.rs`：`Cell { x, z }` / `MoveGoal { cell }` / `step_from_axis`。
- [x] `movement/actions.rs`：`MoveAction { axis }` → `MoveAction { from_cell, to_cell }`；
      执行器改成「设朝向目标格中心的速度 + 挂 `MoveGoal`」。
- [x] `movement/systems.rs`：到格吸附写进 `move_entities_system`；删 `stop_on_round_end_system`。
- [x] `spawn/unit.rs` / `player.rs` / `enemy.rs`：单位带上 `Cell`（由世界坐标 `floor` 取整）。
- [x] 测试：`pressing_walks_exactly_one_cell_and_stops_at_its_center`、
      `move_stays_on_the_ground_and_follows_the_camera`。

### M3 资源与防御 — **完成**

- [x] `combat/defense/stamina.rs`：`Stamina`（+ `try_spend` / `regen`）+ `STAMINA_REGEN_PER_DECISION`
      （**不在 `combat/attributes`**：精力是防御与机动的货币）。
- [x] `combat/defense/`（新子域）：`ParryAction` 载荷、`Dodging` / `Parrying` 标记、
      `roll_executor_system` / `parry_executor_system` / `expire_defense_markers_system`。
      （`RollAction` 载荷住在 `movement`——位移是移动原语。）
- [x] `combat/formula`：防御判定接入（`Dodging` 免伤 / `Parrying` 免伤 + 反制）。
- [x] 测试：`roll_spends_stamina_and_grants_invulnerability`、
      `parry_negates_the_bound_attack_and_counters`、`dodging_negates_damage_until_it_expires`。

### M4 火球与投射物落点 — **完成**

- [x] `combat/skills/fireball.rs`：`FireballAction`（载荷，只管前摇节奏）+ 工厂 + 执行器；
      投射物 `Fireball { target_cell, speed, amount, radius }` **在声明时就生成**。
- [x] `projectile_arrival_system`：按真实距离 ≤ `ARRIVAL_TOLERANCE` 判定到格 → `ProjectileArrived`。
- [x] `explosion_system`：按 `radius` 的**世界距离**取敌对单位 → `DamageEvent`；打空也销毁投射物。
- [x] 测试：`fireball_flies_to_the_locked_cell_and_explodes`、`fireball_whiffs_on_empty_ground`、
      `explosion_hits_only_hostiles_and_always_despawns_the_shell`。

### M5 两阶段结算与领域纯函数 — **完成**

- [x] `combat/formula/domain.rs`（新，零 Bevy）：`AttackStats` / `resolve_combat` / `resolve_attack` /
      `HitOrder` / `Side` / `resolve_defense` / `counter_damage`（从 B 的 `timeless-domain` 迁入，**11 个单测**）。
- [x] `combat/formula/resolution.rs`：`phase1_arbitrate_system`（只读 → `CombatResult`）+
      `phase2_apply_system`（统一应用），经 **`Arbitration` 资源**交接。
- [x] `combat/health`：`apply_damage` 仍是唯一扣血入口，阶段 2 只广播 `DamageEvent`
      （`DamageEvent` → `request_damage_system` → `ModifyHealthEvent` → `apply_damage`）。
- [x] 测试：`domain.rs` 的 11 个纯逻辑用例
      （`layer1_speed_frame_decides_who_hits_first` / `layer2_longer_reach_breaks_frame_tie` /
      `layer3_poise_interrupts_on_full_tie` / `perfect_tie_both_land` / `out_of_range_both_miss` /
      `one_side_out_of_range_hits_alone` / `single_attack_range_check` /
      `range_boundary_is_inclusive` / `dodge_beats_parry_and_landing` /
      `parry_only_negates_the_bound_attack` / `counter_damage_halves_and_never_rounds_to_zero`）
      + 整机用例 `domain_arbitration_orders_by_frame_then_range_then_poise`、
      `phase1_arbitration_does_not_mutate_any_component`、`parry_result_is_decided_in_phase_one`、
      `a_landed_hit_carries_no_counter`、`armor_reduces_physical_damage`、`lethal_damage_triggers_despawn`。

### M6 AI 意图循环 — **完成**

- [x] `ai/systems.rs`：`decide_intent_system`（只读选意图）+ `enemy_declare_system`（声明行动）；
      删 `tick_attack_cooldown_system`（冷却就是动作后摇）。
- [x] `ai/components.rs`：`Intent` 扩到六种（`Idle` / `Approach` / `Melee` / `Shoot` / `Retreat` / `Dodge`），
      `EnemyBrain` 的 `attack_range` 换成 `cautious_health_ratio`（射程归 `AttackRange`）。
- [x] 威胁预判用 `CollisionTarget`（谁瞄准了我），翻滚复用玩家的 `RollCommand` 路径。
- [x] 测试：`choose` 的 6 条分支纯单测（`ai::systems::tests`）
      + 端到端 `enemy_intent_prioritises_dodging_an_incoming_attack`。

### M7 技能菜单 / HUD / 输入 — **完成**

- [x] `combat/skills/registry.rs`：`SkillKind` / `SkillDef` / `SKILLS` / 消耗常量表。
- [x] `combat/skills/menu.rs`：`MenuSelection` + `SelectSkill` / `CycleSkill` / `UseSelectedSkill`
      + 选择 / 循环 / 派发三个系统。
- [x] `input/keyboard.rs`：`1`~`4` 直选、`Tab`/`Shift+Tab` 循环、`G` 释放；
      `Q`/`E`/`Space`/`F`/`V` 保留为快捷施放；`F1` 切 `require_commit`。
- [x] `presentation/hud.rs`：技能行（`>` 选择 / `x` 负担不起）、精力、`Ready/BUSY`、
      草案提示、敌人意图与距离。
- [x] 测试：注册表可用性过滤、菜单越界与循环、`skill_line` 标记、HUD 技能行。

### M8 文档同步与收口 — **进行中**

- [x] `docs/status.md`：重写为进度与决策记录（D1–D5 已决；第一版的冲突表已归档）。
- [x] `docs/design/game-design.md` / `architecture.md`：加描述对象横幅，标清哪些章节已作废 /
      哪些仍然有效，删掉「We-Go 是现行实现」的表述。
- [x] `docs/design/app-modules.md`：按无回合重写（领域表、循环、组件归属、按键表、目录树）。
- [x] `docs/design/ecs-combat-components.md`：改为描述 A 的新组件集（含消息清单、两阶段、迁移对照）。
- [x] `docs/design/timeline.md` / `timeline-core-design.md`：顶部标注「已被 timeline-turnless.md 取代」，
      并说明 We-Go 阶段机**从未在 A 上落地**。
- [x] `docs/index.md` / `AGENTS.md` / `TODO.md`：补入口与描述对象列、修测试数（A = 97）、修按键与消息名。
- [x] `docs/integration-status.md`：**该文件不存在**（索引里的链接已删除）。
- [ ] 本文件自身的增量维护：每次代码改动后同步第 1 / 4 / 6 节与本节勾选。

---

## 9. 已确认的两个次要点

1. **翻滚 / 招架不吃 `require_commit`。** 防御是反应性操作，若还要 Enter 确认就失去意义；
   `require_commit` 只作用于进攻 / 移动类主动作。**已按此实现**。
2. **一格 = 2.0 世界单位**（体素的两倍）。改这一个常量即可切换；
   若改成「一格 = 1 个体素」，需同时把单位模型缩放改到 ~0.5。**已按 2.0 实现**。
