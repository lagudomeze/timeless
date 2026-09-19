# 技能：静态定义 · 释放条件 · 反制

> ⚠️ **目标设计**：标 🚧 的部分代码里还没有。现状是 `SKILLS` 注册表住在
> `combat/skills/`，且**只有战斗技能**在里面——移动 / 跳跃 / 翻滚各自单干。

技能是**这一手"是什么"**，行动是**这一次"发生了什么"**。两者分开：

| | 是什么 | 住哪 | 能不能序列化 |
| :--- | :--- | :--- | :--- |
| **技能定义** `AbilityDef` | **静态数据**：要什么条件、什么节奏、打哪里、什么代价 | `skills` 域的注册表（将来是 `.ron` 资产） | ✅ **只有数据，没有 `Entity` / 没有闭包** |
| **行动** | **运行时状态**：这一次真的发生了什么 | 独立实体，挂在行动者身上（[timeline.md](timeline.md)） | ❌ 装着实体引用与计时 |

**`skills` 是顶层域，不属于 `combat`**：技能是**所有领域共享的静态目录**，
战斗只是最常读它的那个。**移动 / 跳跃 / 翻滚也是技能**，和火球、横扫、招架
平起平坐——不做特殊处理，没有"技能之外的行动"。

## 一、定义（静态、可序列化）

```rust
pub struct AbilityDef {
    pub id: AbilityId,                    // Move | Jump | Roll | Melee | Shoot | Fireball | Parry …
    pub category: AbilityCategory,        // Movement | Attack | Spell | Posture | Reaction
    pub timing: ActionTiming,             // 前摇 / 后摇 / 打断抗性（数值仍由各域给出，见第三节）
    pub targeting: TargetSelector,        // 需要什么目标
    pub cost: u32,                        // 精力
    pub requirements: Vec<Requirement>,   // 释放条件（类别共享条件之外的）
    pub combat: CombatTags,               // 能不能被打断 / 招架 / 格挡
    pub counter: Option<CounterCost>,     // 能不能当反制手段、当反制要付什么（见第四节）
    pub effects: Vec<Effect>,             // 🚧 后置
    pub phases: Vec<Phase>,               // 🚧 后置（多段技能）
}
```

**为什么强调"静态可序列化"**：

- 技能表是**设计数据**，不是逻辑。将来要能从 `.ron` 加载、能热改、能被工具生成，
  所以 `AbilityDef` 里**不允许出现 `Entity`、`Handle`、闭包或任何运行时状态**——
  一旦出现，它就不再是数据，`.ron` 这条路当场断掉。
- 运行时那一半（这一次是谁、打中了谁、还剩多少前摇）**全部在行动实体上**，
  定义里一个字都不存。
- "这一手的效果怎么落地"由**机制域的载荷 + 执行器**回答（`MoveAction` 归
  [`movement`](domain.md)、`FireballAction` 归 [`combat`](combat.md)），
  定义里只写"效果是什么"这种**数据**（`Effect` 一族，后置）。

## 二、注册表：谁定义、谁交上来

```rust
#[derive(Resource)]
pub struct SkillRegistry(/* AbilityId → AbilityDef */);

/// 各域把自己的静态定义交上来（写：机制域；消费：`skills`）。
#[derive(Message)]
pub struct RegisterAbility(pub AbilityDef);
```

- **注册表只有一个**（住 `skills`），**每个域定义自己的技能**：`movement` 交
  `Move` / `Jump` / `Roll`，`combat` 交 `Melee` / `Shoot` / `Fireball` / `Parry`。
  数值因此仍然归各域（`MOVE_TIMING` 在 `movement`、`FIREBALL_TIMING` 在 `combat`），
  注册表只是**聚合**：技能栏、HUD、`can_cast`、反制建议都从这一份读，
  不在三个地方各写一遍花费与条件。
- 交付走 **Message**（`RegisterAbility`），不是"各域往别人的 Resource 里塞"——
  这是铁律里"别人的内部状态不许碰"的标准解法（[domain.md](domain.md)）。
- **没有全局派发器**：注册表回答"这一手是什么"，不回答"谁来物化它"。
  谁声明、谁物化——`movement` 物化移动，`combat` 物化攻击（见 [timeline.md](timeline.md) 第三节）。
  这也是"移动不做特殊处理"能成立的原因：`AbilityId` 只是**静态目录的键**，
  不是调度表。

## 三、释放条件：集中在 `can_cast`

```rust
pub enum Requirement {
    NotSilenced, NotStunned, NotRooted,
    HasStamina(u32), HasWeapon(WeaponType),
    TargetInRange(f32), TargetIsHostile,
    NotOnCooldown,
}

/// **唯一的条件校验点**：这一次能不能出手。
pub fn can_cast(caster: Entity, ability: AbilityId, world: &World) -> Result<(), CastFailReason>;
```

- **在填意图之前跑一次**（见 [timeline.md](timeline.md) 第三节）；失败就写一条
  `ActionBlocked { reason }` 给 HUD——玩家按键失败与 AI 决策失败走同一条路。
- **类别共享条件先判**（`AbilityCategory` 一级），技能只写自己的 `requirements`。
  避免"每个技能把沉默 / 眩晕 / 冷却各写一遍"。
- 它和 [`first_ready`](../src/timeline/decision.rs) 分工明确：

  | | 管什么 | 归谁 |
  | :--- | :--- | :--- |
  | `first_ready` | 这个单位**此刻能不能决策**（决策槽空着吗） | `timeline` |
  | `can_cast` | 这一手**条件够不够**（资源 / 状态 / 距离 / 冷却） | `skills` |

> **已知张力**：`can_cast` 要读 `Stamina`（`combat`）/ `Cell`（`movement`）这些组件。
> 按 import 规则**读组件是允许的**（组件是数据契约），所以现在这么做成立。
> 如果条件族继续膨胀到需要读别人**行为**的程度，就改成"各域注册自己的条件检查器"，
> 那时 `Requirement` 从枚举变成注册表。**现在不预先抽象**。

**为什么条件绑在定义上，而不是"广播意图、各系统否决"**🚧：
RPG 的规则是**设计时已知**的，绑在定义上 → 条件集中、配表可见、失败原因可驱动 UI、
调试直观。代价是新增一类禁止要改 `Requirement` 与 `can_cast`。
（"各系统独立监听并否决"那种消息化解耦适合插件化 / 动态规则，本项目不需要。）

## 四、反制：技能自带的一格数据

**能不能当反制、当反制要付什么，是技能自己的属性**，所以写在定义里：

```rust
pub enum CounterCost {
    Free,                // 白送：反制插入，原决策保留
    Resource(u32),       // 花反制资源（当前就是 Focus），原决策保留
    CancelDecision,      // 拿原决策换：从时间轴移除原决策，插入反制
}
```

- `AbilityDef.counter: Option<CounterCost>`：`None` = 这一手**不能**当反制
  （例如"蓄力三秒的大招"拿来做反制没有意义）。
- 反制的**触发与选择**在 [`combat::reaction`](combat.md)（`ReactionSlot` +
  `CounterSuggestion`）：把当前**所有能当反制的技能**连同它的 `CounterCost`
  一起列给玩家，够不够付由 HUD 标出来。
- 反制**是独立资源、不覆盖决策槽**——反应槽与决策槽共存，互不改结构。

## 五、目标选择

```rust
pub enum TargetSelector {
    SelfOnly,                 // Buff
    TargetEntity,             // 指向性施法
    TargetCell,               // 锁一格（火球落点）
    Direction,                // 射箭 / 枪械
    MeleeArc,                 // 近战扇形
}
```

**选目标只产出"打哪儿"，命中的几何判定归 [combat.md](combat.md)**：
决策层用格（`Cell`），结算层用**形状**（`Shape`，见下节）。

## 六、范围与形状：`utils` 里的纯几何

攻击范围 / 影响范围只是**几何**，不是领域：`Shape` 住在 `utils`，谁都能直接引用
（[domain.md](domain.md) 的 import 规则第 3 条）。

```rust
// src/utils/shape.rs（没有 Plugin、没有组件、纯数据 + 纯函数）
pub enum Shape {
    Point,                    // 单体：伤害、位移落点
    Square { half: f32 },     // 正方形范围：回血、增益光环
    Circle { radius: f32 },   // 爆炸半径
    Arc { radius: f32, half_angle: f32 },  // 近战扇形
}
```

- **具体实现直接引用它，或者包一层**：伤害就是 `Point`，回血就是正方形
  `Square`——`Shape` 不区分"伤害 / 回血"，它只回答"覆盖哪些格 / 哪些点"。
- `TargetSelector` 回答"要什么目标"（设计意图），`Shape` 回答"覆盖多大"
  （几何计算），两者不合并。
- `Threatens { cells }`（[combat.md](combat.md)）是**决策层**的格子集合，
  由 `Shape` 在格尺度上算出来；结算仍用真实距离。

## 七、阶段与效果（🚧 后置）

现在的模型是**固定的三段**，由数据表达而不是新类型：

```text
前摇 = ActionTiming.windup      → ScheduledAction.execute_at 之前
执行 = 到点那一次
后摇 = ActionTiming.recovery    → 决策槽的 Executing { until }
```

**够用就不加机制。** 等到出现"多段技能"（蓄力两段、连击三下、引导型法术）时再引入：

```rust
pub struct Phase {              // 🚧
    pub duration: f32,
    pub on_enter: Vec<SkillEvent>,
    pub on_exit: Vec<SkillEvent>,
}
```

**阶段只描述"何时、发什么事件"，不写逻辑**；逻辑住在订阅 `SkillEvent` 的 Observer 里。

## 八、加一个技能要动哪里

以"冲刺（位移 2 格）"为例：

| 步骤 | 落点 |
| :--- | :--- |
| 1. 定义 | `movement` 加一个常量 `AbilityDef`（category = Movement，timing，requirements，cost，counter）并通过 `RegisterAbility` 交上去 |
| 2. 载荷 | 本域加一个载荷组件（复用 `MoveAction` 也可以） |
| 3. 工厂 | 加场景工厂：载荷 + `ActionTiming` + `ScheduledAction` + `ActionOf(actor)` |
| 4. 执行器 | 加执行器：`due(now)` → 落地效果 → 销毁行动实体 → 槽置 `Executing { until }` |
| 5. 声明入口 | 本域的声明系统：`can_cast` 通过后填 `Intent` 并立即物化（第 3 步的工厂） |

**时间线一行都不用改，注册表也只多一条数据**——这正是"调度器不感知载荷"的意思。
