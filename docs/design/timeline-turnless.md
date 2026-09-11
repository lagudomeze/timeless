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
| **D1** | 保留 **A** 为主线树，B 的能力迁进来 | 目录/分层/BSN/49 个测试基座不动；B 的战斗能力重写为 A 的领域化形态 |
| **D2** | **无回合**：所有 PC/NPC「能决策就决策」；仅当**玩家等待输入**时冻结虚拟时间；默认输入**直接生效**，另有 `require_commit` 开关 | 删除 `Phase` / `round` / `RESOLUTION_WINDOW` / `RoundEnded` / `pause_during_planning_system` |
| **D3** | 保留 A 的**真实距离**结算；**决策**与同格判定按**格子** | 单位/行动落在格上，命中/射程/爆炸用世界距离 |
| **D4** | 节奏 = **固定冷却**：每个动作自带**前摇 + 后摇** | 动作表决定出手快慢；速度属性暂不引入 |
| **D5** | 投射物**锁定目标格** → 自由飞行 → 到达后按真实距离结算 AoE | 弹道为实体运动（A 已有），落点检定与 AOE 半径用世界距离 |

> ⚠️ 旧阶段机（`Phase` / `Timeline` / `window` / 提交窗口）**彻底删除**。
> 注意：本检出**没有 git 历史**（工作区不是 git 仓库），因此「保留为历史提交记录」不可行——
> 旧形态在本文件第 7 节的映射表与 `../status.md` 第二节里留档。

---

## 1. 两条正交的坐标轴（本设计的核心）

最容易搞混的一点：**决策在格子上，结算在真实空间里**。两者用格子边长换算。

```text
决策层（格子，整数）                      结算层（世界，连续）
─────────────────────────                ─────────────────────────
Cell(IVec2)  = 单位在哪一格               Transform.translation: Vec3
step_toward / 同格判定 / 逼近 / 射程格数  距离 / 碰撞半径 / AoE 半径 / 弹道飞行
```

- **格子边长** `CELL_SIZE = 2.0`（世界单位/格）。选 2.0 而不是 1.0 的理由：体素是 1×1×1，
  若 `CELL_SIZE = 1.0`，格子与体素一一对应会把单位压成 1 米大小，和现有 21×21 视觉尺度、
  单位模型缩放对不上；2.0 让「一格 = 两步体素」，单位模型可正常缩放。
- **换算只有一个入口**：`cell::CENTER_OFFSET` 与 `world::cell_center(cell) -> Vec3`。
  任何地方都不许手写 `cell as f32 * 2.0`。
- **两个坐标必须同步**：`Cell` 由「单位行动结束时吸附到格中心」维护，
  不每帧从 `Transform` 反推（避免浮点抖动导致格子跳变）。
  校验入口：`sync_cell_from_transform`（仅在单位停下时调用）。

**射程怎么写**：`AttackRange(cells: u32)`（格数）在结算时换算成世界距离
`range_world = cells as f32 * CELL_SIZE`，命中判定仍是「真实距离 ≤ range_world」。
这样设计稿里的「射程 1 格」和 A 的几何命中能同时成立。

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
/// 动作的固定节奏：前摇 + 后摇 + 到达格。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ActionTiming {
    pub windup: f32,       // 声明 → 执行
    pub recovery: f32,     // 执行 → 重新 Ready
    pub arrive_at: f32,    // execute_at + recovery（冗余，便于 HUD 读）
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
```

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

- `waiting_for_input = 玩家有 Ready && 玩家没有未执行的行动`。
- `timeline_gate_system`：`waiting_for_input == true` → `Time<Virtual>::pause()`，否则 `unpause()`。
  这是**全局唯一的暂停点**，各领域依旧不需要任何 `if paused` 分支——
  移动、计时器、后摇、投射物生命周期全部自动停表。

> **语义说明（重要）**：冻结期间 NPC 的后摇也一并不走。这是刻意的：
> 世界是「等玩家想好」而不是「实时压力」。若将来要做真实时压力模式，
> 只需把后摇记在 `Time<Real>` 上，其余不动——这一点作为扩展点明确留出。

### 3.2 默认：输入直接产生效果

`require_commit = false`（默认）时：

```text
按 W        → MoveCommand → declare_move_system → 挂 ScheduledAction（windup 0.15）→ 0.15s 后起步
按 E（近战） → MeleeCommand → 立即执行
按 Q（火球） → FireCommand → 立即朝目标格飞行
```

没有「先声明再确认」这一步；玩家按下就是决定。声明完成后 `Ready` 被移除，
**移动键继续按住会在后摇结束时再次触发**（见 3.4），形成逐格节奏。

### 3.3 开关：`require_commit = true`

```text
按 W        → 只生成 / 覆盖草案（Declared），玩家仍 Ready，时间仍冻结
按 Enter    → ActionsCommitted → 草案升为 Pending → 时间恢复流动
```

- 草案同一时刻至多一条（后声明覆盖先声明——A 现有的 `clear_declared_actions` 已实现该语义，保留）。
- HUD 在草案存在时提示 `DRAFT: move (W) — Enter to commit`。
- 实现位置：`commit_bridge_system` 读一个 `TimelineConfig` 资源，**不散落到各领域**。

```rust
#[derive(Resource, Debug)]
pub struct TimelineConfig {
    /// true = 需要 Enter 确认；false = 输入立即生效。
    pub require_commit: bool,
    /// 开发用：面板 / 快捷键切换（F1）。
}
```

> 兼容旧行为：把 `require_commit` 设为 true，玩家的手感就与旧 `Planning` 阶段几乎一致，
> 区别只是 NPC 不再等玩家（它们按自己的冷却走）。

### 3.4 按住键的自动重复

`input` 只在**方向变化**时发一次 `MoveCommand`（`Local` 记住上次方向），
`declare_*_system` 也只处理有 `Ready` 的单位。于是：

- **按一次 W = 走一格**（决策按格，不需要连发消息）；
- 按住 W 不会每帧重复声明，因此**不会顶掉**玩家刚按下的技能键；
- 想连续走两格：松开再按一次（一格一次决策）。

> 为什么不像旧模型那样每帧写消息：无回合模型里「按住」是一种持续状态，
> 而决策是离散事件；把两者混在一起会让技能键被移动键瞬间覆盖。
> 这是与旧「规则 3.4」不同的地方，也是格子决策模型的直接推论。

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
| 跳跃 `JumpAction` | 0.10s | 落地后 0.05s | 沿用 A 现有弹道 |
| 近战 `MeleeAction` | 0.20s | 0.35s | 出手快、硬直长 |
| 火球 `FireballAction` | 0.30s | 0.50s | 出手慢、威力大 |
| 翻滚 `RollAction` | 0.05s | 0.30s | 防御性，几乎立即生效 |
| 招架 `ParryAction` | 0.05s | 0.25s | 同上 |

数值验收方式：一次交锋内「我打两下、敌人打一下」这类节奏由这些常量决定，
后续调参只改这张表（配 `Command`/`SKILLS` 消耗）。

---

## 5. 移动：格为目标，连续插值

```rust
/// 移动载荷：朝 `axis`（归一化平面方向）走一格。
#[derive(Component)] pub struct MoveAction { pub to_cell: IVec2 }

/// 单位当前所在格（决策层坐标）。
#[derive(Component)] pub struct Cell(pub IVec2);
```

- **声明**：`from = Cell.0`；`to_cell = from + step(axis)`，`step` 由
  [`step_from_axis`](../../src/movement/actions.rs) 把任意平面方向吸附成
  **正交的一格**（`(±1,0)` / `(0,±1)`）：斜向输入取绝对值大的分量，
  分量相等时固定走 Z。决策永远是「走一格」，不存在半格。
- **执行**：给行动者 `Velocity = (cell_center(to_cell) - Transform.translation).normalize() * move_speed`
  与 `MoveGoal { cell: to_cell }`。
- **停下**：新增 `arrive_at_goal_system`：位移到格中心（或越过）时
  `translation = cell_center(to_cell)`、`Velocity = 0`、`Cell = to_cell`、移除 `MoveGoal`。
  于是「走到哪停哪」，不依赖任何窗口。
- **跳跃**：保留 A 的 `Jumping` 弹道（`v += g·dt`），落地时同样吸附到 `cell_center(Cell)`。

---

## 6. B 的能力迁移：逐项落点

### 6.1 精力（`Stamina`）

```rust
#[derive(Component)] pub struct Stamina { pub current: u32, pub max: u32 }
/// 恢复到 Ready 时回复的精力（取代 B 的「每轮回 1」）。
pub const STAMINA_REGEN_PER_DECISION: u32 = 1;
```

归属：`combat/attributes`（和 `Armor` / `HitRadius` 同层）。
回复时机：`recovery_system` 恢复 `Ready` 的那一次结算里 +1（上限 `max`）；
不再有「回合结束」事件可挂，这也是删 `RoundEnded` 的连带影响之一。

### 6.2 翻滚（`Roll` + `Dodging`）— **已落地**

- 载荷 `RollAction { from_cell, to_cell }` 住在 [`crate::movement`]（位移是移动的原语），
  声明与落地在 `combat/defense/actions.rs`。
- 声明：`F` 键 → 远离最近敌对单位的方向 → `step_from_axis` 吸附成一格 → 1 精力。
- 执行：`Velocity` 指向目标格中心 → 复用第 5 节的到格逻辑 → 同时挂
  `Dodging { expires_at: now + DODGE_SECS }`（`DODGE_SECS = 0.5`，对齐 B 的 `DODGE_MS = 500`）。
- 清理：`expire_defense_markers_system` 到点移除。

### 6.3 招架（`Parry` + `Parrying`）— **已落地**

- 载荷 `ParryAction { target_attack }`：指向**当前还在 `Declared` 的动作实体**（威胁）。
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
phase1_arbitrate_system（Local<Vec<CombatResult>>）
    只读裁决：三层裁决 + 防御判定 → 最终伤害（快照一致，谁先算谁不算占便宜）
phase2_apply_system
    统一落地：DamageEvent / 招架反制 / 命中计数 + finished / 清 CollisionTarget
        ↓
    request_damage_system → apply_damage（**唯一**扣血入口）
```

- 纯逻辑放 `combat/formula/domain.rs`（**零 Bevy 依赖**，可脱离 App 单测）：
  `AttackStats { frame, range, impact, damage }` · `resolve_attack` · `resolve_combat` ·
  `HitOrder` / `Side` / `HitResult` · `DefenseState` + `resolve_defense` + `counter_damage`。
- **三层裁决按 A 的坐标模型改写**：B 的第二层是「格距离」，A 是「真实距离」——
  L1 `AttackFrame` 小者先 → L2 `AttackRange::world()` 大者先 → L3 `Impact` 大者打断小者。
- 配置：近战 帧 5 / 破势 3；箭矢 帧 4 / 破势 1；火球 帧 7 / 破势 2。
- 隔离缓冲用 `Local<Vec<CombatResult>>`（同帧强顺序的一对系统），不用消息——
  项目的 `Event` + `Observer` 约定只服务于「即时、定向实体」的响应。
- **同刻互击**：`phase1` 先做一遍**参数快照**（`(攻击实体, 目标, 真实距离) → AttackStats`），
  两边看到的是同一份数据，各自的裁决互为镜像（A→B 的 `Side::Attacker` 即 B→A 的 `Side::Defender`）。
  被破势打断的一方伤害归零、`DefenseOutcome` 改写为 `Interrupted`（日志要留下这次交锋）。
- `DamageEvent` 仍然只由 `apply_damage` 消费，因此治疗 / 中毒 / 再生等入口不受影响。
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
| `RoundEnded` / `round()` | **删除** | 需要计步时用「决策次数」而非轮次；HUD 显示 `DECISIONS` |
| `commit_actions_system` / `ActionsCommitted` | **保留**（默认旁路） | 仅 `require_commit = true` 时参与 |
| `pause_during_planning_system` | `timeline_gate_system` | 暂停条件从「规划阶段」变成「玩家等待输入」 |
| `execute_at = 提交时刻 + 前摇` | `execute_at = 声明时刻 + 前摇` | 声明即开始前摇（无回合） |
| 逻辑刻度 `hit_clock` / `GlobalTime` | `execute_at`（虚拟秒）+ 领域层破平 | 「帧」= `execute_at` 排序键，不再引入第二套时钟 |
| `PendingHit` 实体 | `Projectile` + `ArrivesAtCell` + `ProjectileArrived` | 到达后按真实距离结算，等同物化延迟命中 |
| `AttackRange(u32)` 切比雪夫 | `AttackRange(cells)` → 世界距离 | 结算用真实距离（D3） |
| `MoveTo { velocity: IVec2 }` | `MoveAction { to_cell }` + 世界插值 | 决策按格、表现连续 |
| `Position(IVec2)` | `Cell(IVec2)` 双层坐标 | 见第 1 节 |
| `CancelPrivilege` / `try_cancel` | `Ready` 门控 + 前摇窗口内可声明翻滚 | 无回合下「取消」= 在前摇未到点前用新行动覆盖，天然无需特权组件 |

---

## 8. 迁移路线图（每步保持可编译 + 测试全绿）

> ⚠️ 本检出无法运行 `cargo`（shell 被沙箱拒绝），每步的验证需你本地执行。
> 命令见 [../status.md](../status.md) 第八节。

### M1 时间线改造（地基）

- [ ] `timeline/resources.rs`：删 `Phase` / `round` / `window` / `RESOLUTION_WINDOW`；
      换成 `Timeline { waiting_for_input, draft }` + `TimelineConfig { require_commit }`。
- [ ] `timeline/components.rs`：`ScheduledAction` 加 `timing` / `declared_at`；新增 `Ready`、`BusyRecovery`、`ActionTiming`。
- [ ] `timeline/systems.rs`：删 `pause_during_planning_system` / `end_round_system`；
      新增 `timeline_gate_system` / `commit_bridge_system` / `recovery_system`；`scheduler_system` 改用 `Timeline` 新字段。
- [ ] `timeline/events.rs`：删 `RoundEnded`；`ActionsCommitted` 保留（受 `require_commit` 控制）。
- [ ] `timeline/timing.rs`（新）：第 4 节的常量表。
- [ ] `timeline/resources.rs` 里 4 个旧单测重写为：`ready_units_are_gated_by_player_input`、
      `recovery_restores_ready`、`require_commit_defers_execution`、`input_directly_executes_by_default`。
- [ ] 全局替换：所有 `timeline.is_planning()` → 「actor 有 `Ready`」查询。
      涉及：`movement/actions.rs`（3 处）、`combat/skills/actions.rs`、`ai/systems.rs`、`spawn/restart.rs`、`presentation/hud.rs`。

### M2 双层坐标与格子移动

- [ ] `world`：导出 `CELL_SIZE` / `cell_center` / `cell_of` / `nearest_walkable_cell`（供翻滚与 AI 用）。
- [ ] `movement/components.rs`：新增 `Cell(IVec2)`、`MoveGoal { cell }`。
- [ ] `movement/actions.rs`：`MoveAction { axis }` → `MoveAction { to_cell }`；
      执行器改成「设朝向目标格的速度 + 挂 `MoveGoal`」。
- [ ] `movement/systems.rs`：新增 `arrive_at_goal_system`；删 `stop_on_round_end_system`。
- [ ] `spawn/unit.rs` / `player.rs` / `enemy.rs`：单位带上 `Cell`（由世界坐标取整）。
- [ ] 测试：`holding_the_key_steps_one_cell_per_recovery`、
      `unit_snaps_to_cell_center_on_arrival`、`cell_and_transform_stay_in_sync`。

### M3 资源与防御

- [ ] `combat/attributes`：`Stamina`（+ `try_spend`）+ `STAMINA_REGEN_PER_DECISION`。
- [ ] `combat/defense/`（新子域）：`RollAction` / `ParryAction` 载荷、`Dodging` / `Parrying` 标记、
      `roll_executor` / `parry_executor` / `expire_defense_markers_system`。
- [ ] `combat/formula`：防御判定接入（`Dodging` 免伤 / `Parrying` 免伤 + 反制）。
- [ ] 测试：`roll_grants_invulnerability_until_expiry`、`parry_negates_bound_attack_and_counters`、
      `stamina_is_spent_and_regenerates_on_ready`。

### M4 火球与投射物落点

- [ ] `combat/skills`：`FireballAction { target_cell }` + 工厂 + 执行器（生成投射物）。
- [ ] `combat/lifecycle`：`ArrivesAtCell(IVec2)` + `projectile_arrival_system` → `ProjectileArrived`。
- [ ] `combat`：`explosion_system`（按 `ExplosionRadius` 世界距离，复用 `apply_hit`）。
- [ ] 测试：`fireball_explodes_at_locked_cell`、`fireball_whiffs_when_nobody_is_in_radius`、
      `explosion_uses_world_distance_not_cell_distance`。

### M5 两阶段结算与领域纯函数

- [ ] `combat/formula/domain.rs`（新，零 Bevy）：`AttackStats` / `resolve_combat` / `resolve_attack` /
      `HitOrder` / 反制伤害 —— 从 B 的 `timeless-domain` 迁入，测试一并迁移（7 个）。
- [ ] `combat/formula`：`phase1_arbitrate_system`（只读 → `CombatResult`）+ `phase2_apply_system`（统一应用）。
- [ ] `combat/health`：`apply_damage` 改为消费 `CombatResult` 的最终伤害（单一扣血入口）。
- [ ] 测试：`simultaneous_hits_are_resolved_by_poise`、`dodge_negates_in_phase1`、
      `phase1_does_not_mutate_any_component`。

### M6 AI 意图循环

- [x] `ai/systems.rs`：`decide_intent_system`（只读选意图）+ `enemy_declare_system`（声明行动）；
      删 `tick_attack_cooldown_system`（冷却就是动作后摇）。
- [x] `ai/components.rs`：`Intent` 扩到六种（`Idle` / `Approach` / `Melee` / `Shoot` / `Retreat` / `Dodge`），
      `EnemyBrain` 的 `attack_range` 换成 `cautious_health_ratio`（射程归 `AttackRange`）。
- [x] 威胁预判用 `CollisionTarget`（谁瞄准了我），翻滚复用玩家的 `RollCommand` 路径。
- [x] 测试：`choose` 的 6 条分支纯单测 + 端到端「有攻击打向自己时选 Dodge」。

### M7 技能菜单 / HUD / 输入

- [x] `combat/skills/registry.rs`：`SkillKind` / `SkillDef` / `SKILLS` / 消耗常量表。
- [x] `combat/skills/menu.rs`：`MenuSelection` + `SelectSkill` / `CycleSkill` / `UseSelectedSkill`
      + 选择 / 循环 / 派发三个系统。
- [x] `input/keyboard.rs`：`1`~`4` 直选、`Tab`/`Shift+Tab` 循环、`G` 释放；
      `Q`/`E`/`Space`/`F`/`V` 保留为快捷施放；`F1` 切 `require_commit`。
- [x] `presentation/hud.rs`：技能行（`>` 选择 / `x` 负担不起）、精力、`Ready/BUSY`、
      草案提示、敌人意图与距离。
- [x] 测试：注册表可用性过滤、菜单越界与循环、`skill_line` 标记、HUD 技能行。

### M8 文档同步与收口

- [ ] `docs/status.md`：D1–D3 标为已决；冲突表 C1/C2/C4/C6/C9/C10 标注「已由本设计解决 / 待随代码收口」。
- [ ] `docs/design/game-design.md` / `architecture.md`：按本设计改写「核心循环」章节，删掉「逻辑刻度」表述。
- [ ] `docs/design/app-modules.md`：更新 `timeline` 域说明（无阶段机）与按键表（Q 射击 / Space 跳跃）。
- [ ] `docs/design/ecs-combat-components.md`：改为描述 A 的新组件集（本设计第 2、6 节）。
- [ ] `docs/design/timeline.md` / `timeline-core-design.md`：顶部标注「已被 timeline-turnless.md 取代」。
- [ ] `docs/index.md` / `AGENTS.md` / `TODO.md`：补入口、修测试数（A 49 + 迁移新增）、修按键与消息名。
- [ ] `docs/integration-status.md`：删除或降级为历史。

---

## 9. 待你确认的两个次要点

1. **翻滚/招架是否也吃 `require_commit`？** 建议：**不吃**。防御是反应性操作，
   若还要 Enter 确认就失去意义；`require_commit` 只作用于进攻/移动类主动作。
2. **一格 = 2.0 世界单位**（体素的两倍）。若你希望「一格 = 1 个体素」，
   需同时把单位模型缩放改到 ~0.5——我按 2.0 实现，改这一个常量即可切换（第 1 节）。
