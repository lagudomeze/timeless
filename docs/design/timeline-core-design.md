# 时间线核心（Timeline Core）设计文档 — v0.1

> 状态：**接口签名版**（只定义 API 与模块划分，不写实现）
> 技术栈：Bevy (Rust, ECS)
> 范围：第一梯队 —— 攻击类（轻击/蓄力/远程射击）+ 翻滚取消 + 基础移动 + 弹药装填，
> 并预留 反应/取消类 其余策略（格挡取消/受击闪避/打断）的挂载点。

---

## 1. 设计目标

1. 把「决策/动作提交到时间线」统一为一条管线：**意图 → 合法化检查 → 扣费 → 相位推进 → 判定 → 恢复**。
2. 让「翻滚取消攻击」成为取消机制的**第一个消费者**，把 `try_cancel` 通用 API 定死，后续策略只填参数。
3. 把「玩家独有特权」编码进系统层（`CancelPrivilege` 组件守卫），而不是靠数据约定。
4. 为 `ReactionOpportunityEvent`（决策暂停唯一触发源）预留完整事件与状态机。

## 2. 架构总览：两层时间模型

| 层 | 名称 | 作用 | 载体 |
| :--- | :--- | :--- | :--- |
| L1 | **行动管线** `ActionPipeline` | 每个战斗实体**当前动作**的 前摇→判定帧→后摇 推进 | Entity Component |
| L2 | **全局战斗时间线** `CombatTimeline` | 未来事件（弹道飞行、延迟 AOE、地面效果）按时间戳排程 | Resource |

- 玩家输入/怪物 AI 只产生**意图**（`ActionIntent`），是否成为动作由裁决器决定。
- 决策暂停（`CombatPhase::DecisionPause`）时**冻结 L1 与 L2 的时钟**，恢复后继续。

### 模块划分

```
src/
  timeline_core/     # 引擎无关内核：Phase / ActionDef / Cancel / CombatTimeline
    mod.rs
    action.rs        # ActionId, PhaseKind, ActionPhase, ActionDef, Cost
    cancel.rs        # CancelRule, try_cancel, CancelError
    timeline.rs      # CombatTimeline, ScheduleEntry, TimedEvent
  combat/            # 判定帧裁决：伤害 / 破势 / 架势槽（第二梯队扩展）
  actions/           # ActionDef 注册表：轻击/蓄力/翻滚/移动/装填/射击
  reaction/          # 决策暂停状态机 + ReactionOpportunityEvent
  resources/         # Stamina / AmmoPouch / Cooldowns / Poise 组件
  input/             # 键盘 → ActionIntent
  main.rs
```

## 3. 核心类型与接口签名

### 3.1 动作定义层（timeline_core::action）

```rust
/// 动作标识：注册表主键
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ActionId(pub &'static str);
// 预定义：LIGHT_ATTACK, HEAVY_ATTACK, RANGED_ATTACK, ROLL, ROLL_CANCEL,
//         MOVE, RELOAD, GUARD（第二梯队）, INTERRUPT（第二梯队）

/// 相位种类
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PhaseKind { Startup, Active, Recovery }

/// 单个相位的推进状态
#[derive(Clone, Copy, Debug)]
pub struct ActionPhase {
    pub kind: PhaseKind,
    pub elapsed: f32,
    pub duration: f32,   // 秒
}

/// 资源消耗 —— 规则：在 Startup 开始时一次性扣除，取消不退还
#[derive(Default, Clone, Copy, Debug)]
pub struct Cost {
    pub stamina: u32,                       // 精力
    pub ammo: u32,                          // 弹药
    pub special: Option<ItemId>,            // 特殊材料（破甲攻击用，预留）
}

/// 动作定义（注册表条目，纯数据）
pub struct ActionDef {
    pub id: ActionId,
    pub label: &'static str,
    pub phases: [f32; 3],        // [Startup, Active, Recovery] 时长（秒）
    pub cost: Cost,              // Startup 开始时扣除
    pub cooldown_key: Option<ActionId>,  // 共享冷却键（ROLL 与 ROLL_CANCEL 共用 → 3s）
    pub cooldown: Option<f32>,   // 冷却秒数
    pub priority: u8,            // 抢占优先级（取消裁决用）
    pub cancelable_by: Vec<CancelRule>,   // 可被哪些规则取消（怪物动作留空）
    pub impact: ImpactSpec,      // Active 阶段结束时的效果
    pub move_lock: MoveLock,     // 各阶段移动锁（后摇期间不可移动）
}

/// 判定帧效果
pub struct ImpactSpec {
    pub damage: i32,
    pub poise_damage: u32,            // 破势
    pub knockback: Option<GridDir>,
    pub element: Option<Element>,     // D 层元素反应预留
}

/// 阶段级移动锁
pub struct MoveLock {
    pub startup: bool,    // 前摇锁移动：true（蓄力时不可移动）
    pub active: bool,     // 判定帧锁移动：true
    pub recovery: bool,   // 后摇锁移动：true —— 「攻击后摇期间不可移动」
}
```

### 3.2 行动管线（L1）

```rust
/// 挂在每个战斗实体上：当前动作 + 备用队列
#[derive(Component, Default)]
pub struct ActionPipeline {
    pub current: Option<ActiveAction>,
    pub queue: VecDeque<QueuedIntent>,   // 预留：连招/队列扩展
}

pub struct ActiveAction {
    pub def: Arc<ActionDef>,
    pub phase: ActionPhase,
    pub inputs: ActionInputs,     // 方向 / 目标 Entity
    pub started_at: f32,          // 扣费时间点（取消不退还的凭据）
}

/// 相位转换事件（UI、音效、取消窗口检测都监听它）
#[derive(Event)]
pub struct PhaseTransitionEvent {
    pub actor: Entity,
    pub action: ActionId,
    pub from: PhaseKind,
    pub to: PhaseKind,
}
```

### 3.3 全局战斗时间线（L2）

```rust
/// 未来事件排程 —— 弹道飞行、延迟 AOE、地面效果
#[derive(Resource, Default)]
pub struct CombatTimeline {
    pub clock: f32,                       // 战斗时钟（DecisionPause 时冻结）
    pub scheduled: BinaryHeap<ScheduleEntry>,   // 按 at 升序（反转堆序）
}

pub struct ScheduleEntry {
    pub at: f32,
    pub kind: TimedEvent,
    pub origin: Entity,
}

pub enum TimedEvent {
    ProjectileImpact { target: Entity, damage: i32, element: Option<Element> },
    AoeDetonation { cell: GridPos, spec: ImpactSpec },
    GroundEffectEnd { cell: GridPos, effect: Element },
}
```

### 3.4 取消机制（timeline_core::cancel）—— 本版核心

```rust
/// 取消规则：某动作的某阶段可被指定动作打断
pub struct CancelRule {
    pub during: PhaseKind,        // 本版只用 Startup（前摇窗口）
    pub by: Vec<ActionId>,        // 允许取消者（如 ROLL）
    pub surcharge: u32,           // 取消附加精力（翻滚取消 = +1，合计 2）
    pub refund_ammo: bool,        // 默认 false —— 「取消时弹药不退还」
}

/// 取消唯一入口。privilege 为 None 时（怪物实体）一律拒绝。
pub fn try_cancel(
    privilege: Option<&CancelPrivilege>,   // 仅玩家实体携带此组件
    current: &ActiveAction,
    incoming: &ActionDef,
    resources: &mut ActorResources,        // 精力/冷却查询+扣费
) -> Result<CancelOutcome, CancelError>;

pub struct CancelOutcome {
    pub stamina_spent: u32,       // 基础 1 + surcharge
    pub refund_ammo: u32,         // 本版恒为 0
    pub cooldown_keys: Vec<ActionId>,  // 翻滚 CD 从此刻起算 3s
}

pub enum CancelError {
    NotCancelable,        // 当前阶段/动作没有匹配的 CancelRule
    PrivilegeDenied,      // 非玩家实体试图取消（怪物无取消权）
    InsufficientStamina,
    CooldownActive,       // 翻滚 3s CD 未结束
}

/// 玩家独有特权标记：仅 add 到玩家实体
#[derive(Component)]
pub struct CancelPrivilege;
```

### 3.5 反应 / 决策暂停（reaction）

```rust
/// 战斗阶段状态机：决策暂停时冻结两层时间线
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CombatPhase {
    #[default] Running,       // 时间线推进
    DecisionPause,            // 冻结，等待玩家选择
}

/// 决策暂停的唯一触发源 —— 「攻击前摇中 + 检测到威胁」
#[derive(Event)]
pub struct ReactionOpportunityEvent {
    pub actor: Entity,
    pub source_action: ActionId,     // 被取消目标（必然处于 Startup）
    pub options: Vec<ReactionOption>,
}

pub struct ReactionOption {
    pub cancel_with: ActionId,       // ROLL / GUARD(二梯队)
    pub surcharge: u32,              // 精力附加
    pub shared_cooldown: ActionId,   // ROLL：与基础翻滚共用 3s CD
}

/// 威胁检测接口：基础实现用视野范围；洞察力（G 层）解锁帧数细节
pub trait ThreatDetector {
    fn scan(&self, ctx: &CombatContext, actor: Entity) -> Vec<ThreatInfo>;
}
```

### 3.6 资源组件（resources）

```rust
#[derive(Component)]
pub struct Stamina { pub current: u32, pub max: u32 }   // 恢复速率在 resource_system

#[derive(Component, Default)]
pub struct AmmoPouch { pub ammo: HashMap<AmmoType, u32> }  // 已装填弹药栏

#[derive(Component, Default)]
pub struct Cooldowns { pub remaining: HashMap<ActionId, f32> }  // 键含 ROLL

#[derive(Component)]
pub struct Poise { pub current: u32, pub max: u32 }   // 架势槽（格挡/破甲，二梯队）
```

### 3.7 意图与裁决入口（input + intent）

```rust
/// 玩家输入 / 怪物 AI 统一产生的意图
#[derive(Event)]
pub struct ActionIntent {
    pub actor: Entity,
    pub action: ActionId,
    pub dir: Option<GridDir>,
    pub target: Option<Entity>,
}

/// 意图合法化：资源检查 → CD 检查 → 当前阶段检查 → 扣费 → 生成 ActiveAction
pub fn try_start_action(
    def: &ActionDef,
    pipeline: &ActionPipeline,
    resources: &mut ActorResources,
) -> Result<ActiveAction, IntentError>;

pub enum IntentError {
    Busy,                 // 处于不可被打断的 Active/Recovery 阶段
    InsufficientStamina,
    InsufficientAmmo,
    CooldownActive,
    MoveLocked,           // 后摇期间移动被锁
}
```

## 4. 表格策略 → 代码映射（第一梯队）

| 策略 | ActionDef / 规则 | 关键参数（来源：策略表） |
| :--- | :--- | :--- |
| 轻击 | `LIGHT_ATTACK` | phases ≈ [0.15, 0.1, 0.3]；cost 微量；后摇锁移动 |
| 蓄力重击 | `HEAVY_ATTACK` | 长 Startup（判定帧靠后）；cost 弹药×2+精力；取消不退还弹药 |
| 远程射击 | `RANGED_ATTACK` | 判定不立即执行 → 往 `CombatTimeline` 排 `ProjectileImpact`（飞行时间=延迟命中，可被闪避） |
| 翻滚取消攻击 | `CancelRule{ during: Startup, by: [ROLL], surcharge: 1, refund_ammo: false }` | 共用 `cooldown_key: ROLL`（3s）；与基础翻滚共用 CD |
| 基础移动 | `MOVE` | 由 `MoveLock` 保证后摇期间不可移动 |
| 使用/装填弹药 | `RELOAD` | 背包 → `AmmoPouch`；受装填速度影响（后续加 ActionDef 变体） |

取消时序（本版唯一博弈闭环）：

```
轻击 Startup 开始 ──扣费(弹药×1, 不退还)──▶ 威胁检测
      │                                      │ 命中 → ReactionOpportunityEvent
      │                                      ▼
      │                          CombatPhase::DecisionPause（时钟冻结）
      │                                      │
      │                 ┌──选择「翻滚取消」──▶ try_cancel(privilege=Some) → 中断
      │                 │                    翻滚动作入管线，带无敌帧，CD 3s 起算
      └── 无威胁/放弃 ──┘                    弹药已扣，不退还（取消惩罚）
      ▼
 Active(判定帧) → Recovery(后摇, 锁移动) → 回到 Idle
```

## 5. 玩家独有特权的编码（两处防线）

1. **系统层（硬规则）**：`try_cancel` 要求 `Option<&CancelPrivilege>`，怪物实体没有该组件 → `PrivilegeDenied`。这是「怪物不能取消已锁定动作」的实现位置。
2. **数据层（约定）**：怪物注册表（`EnemyActions`）的 ActionDef 一律 `cancelable_by: vec![]`。

同样玩家独有：查看日志 / 死亡复盘 / 洞察力 → G 层，本版不实现，只留 `ThreatDetector` 接口。

## 6. Bevy 系统与调度（FixedUpdate）

| 系统 | 职责 | 运行条件 |
| :--- | :--- | :--- |
| `input_system` | 键盘 → `ActionIntent`（含翻滚取消意图） | `CombatPhase::Running` |
| `ai_intent_system` | 怪物 → `ActionIntent`（走同一条裁决管线） | `Running` |
| `intent_system` | `try_start_action` / `try_cancel` 裁决 | `Running` |
| `phase_system` | 推进 `ActionPhase`；发 `PhaseTransitionEvent` | `Running` |
| `impact_system` | Active 结束 → 执行 `ImpactSpec`，弹道排入 L2 | `Running` |
| `timeline_system` | 推进 `CombatTimeline.clock`，触发到点事件 | `Running` |
| `reaction_system` | 检测「Startup + 威胁」→ 发事件 + 切 `DecisionPause` | `Running` |
| `resolve_choice_system` | 玩家选择 → 执行取消/放弃 → 回 `Running` | `DecisionPause` |
| `resource_system` | 精力恢复 / 冷却递减 / 弹药扣费 | 始终 |

## 7. 未决问题（v0.2 前需定）

1. **决策暂停的冻结范围**：全局冻结（L1+L2 都停，回合感强，默认）vs 局部冻结（仅玩家管线停，怪物继续——更刺激但需要「威胁实时性」设计）。
2. **蓄力重击的取消窗口**：蓄力中是否可翻滚取消？（黑神话可以，代价是蓄力进度丢失）建议：可以，但蓄力进度归零。
3. **装填速度**：`RELOAD` 用独立 ActionDef（长 Startup）还是资源系统里的纯计时器？建议前者——可被打断（受击中断装填）。
4. **翻滚的无敌帧**：挂在 ROLL 的 Active 阶段（0.3s 左右），`InvulnerableFrame` 标记，受击闪避（二梯队）复用。

## 8. 下一步

1. 按本签名搭建 `cargo new` + Bevy 依赖，让全部签名可编译（TODO 空实现）。
2. 先实现 `phase_system` + `intent_system` 闭环（轻击可执行、后摇锁移动）。
3. 再实现 `reaction_system` + `try_cancel`（翻滚取消闭环，验证决策暂停）。
4. 二梯队按同一接口填：格挡取消 = 在 Startup 加一条 `CancelRule{ by: [GUARD] }`，受击闪避 = 敌方 Active 帧触发（需洞察力信息，G 层）。
