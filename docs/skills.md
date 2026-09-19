# 技能：定义 · 释放条件 · 行动

> ⚠️ **目标设计**：标 🚧 的部分代码里还没有（现状是 `SKILLS` 注册表 +
> 各声明系统各判各的条件）。

分清两件事：

| | 是什么 | 住哪 |
| :--- | :--- | :--- |
| **技能定义** `AbilityDef` | **静态数据**：这一手要什么条件、什么节奏、打哪里、什么效果 | 注册表 / 将来的 `.ron` 资产 |
| **行动** | **运行时状态**：这一次真的发生了什么 | 独立实体（见 [timeline.md](timeline.md)） |

技能本身**没有空间位置**，不挂 `ChildOf`（见 [relations.md](relations.md)）。

## 一、定义

```rust
pub struct AbilityDef {
    pub id: AbilityId,
    pub category: AbilityCategory,        // Spell | Attack | Movement | Posture
    pub timing: ActionTiming,             // 前摇 / 后摇 / 打断抗性
    pub targeting: TargetSelector,
    pub cost: u32,                        // 精力
    pub requirements: Vec<Requirement>,   // 释放条件（类别条件之外的）
    pub combat: CombatTags,               // 能不能被打断 / 招架 / 格挡
    pub effects: Vec<Effect>,             // 🚧 后置
    pub phases: Vec<Phase>,               // 🚧 后置（多段技能）
}
```

**具体数值归各域**（`MOVE_TIMING` / `FIREBALL_TIMING`…），注册表只是**聚合展示**：
技能栏、HUD、`can_cast` 都从这里读，不在三个地方各写一遍花费。

## 二、释放条件：集中在 `can_cast`

```rust
pub enum Requirement {
    NotSilenced, NotStunned, NotRooted,
    HasMana(u32), HasWeapon(WeaponType),
    TargetInRange(f32), TargetIsHostile,
    NotOnCooldown,
}

/// **唯一的条件校验点**：这一次能不能出手。
pub fn can_cast(caster: Entity, ability: AbilityId, world: &World) -> Result<(), CastFailReason>;
```

- **在填意图之前跑一次**（见 [timeline.md](timeline.md) 第二节）；失败就写一条
  `ActionBlocked { reason }` 给 HUD —— 玩家按键失败与 AI 决策失败走同一条路。
- **类别共享条件**先判（`AbilityCategory` 一级），技能只写自己的
  `requirements`。避免"每个技能把沉默 / 眩晕 / 冷却各写一遍"。
- 它和 [`first_ready`](../../src/timeline/decision.rs) **分工明确**：

  | | 管什么 | 归谁 |
  | :--- | :--- | :--- |
  | `first_ready` | 这个单位**此刻能不能决策**（决策槽空着吗） | `timeline` |
  | `can_cast` | 这一手**条件够不够**（资源 / 状态 / 距离 / 冷却） | 技能域 |

为什么把条件绑在定义上而不是"广播意图、各系统否决"🚧：
RPG 的规则是**设计时已知**的，绑在定义上 → 条件集中、配表可见、失败原因可驱动 UI、
调试直观。**代价**是新增一类禁止要改 `Requirement` 与 `can_cast`。
（"各系统独立监听并否决"那种消息化解耦适合插件化 / 动态规则，本项目不需要。）

## 三、目标选择

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
决策层用格（`Cell`），结算层用真实距离与形状（`Shape`）。

## 四、阶段与效果（🚧 后置）

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
pub struct SkillTimeline { phases: Vec<Phase>, current: usize, elapsed: f32 }  // 🚧
```

**阶段只描述"何时、发什么事件"，不写逻辑**；逻辑住在订阅 `SkillEvent` 的 Observer 里。
这样加一个新阶段不需要改时间线。

## 五、加一个技能要动哪里

以"冲刺（位移 2 格）"为例：

| 步骤 | 落点 |
| :--- | :--- |
| 1. 定义 | 注册表加一条 `AbilityDef`（category = Movement，timing，requirements，cost） |
| 2. 载荷 | 本域加一个载荷组件（复用 `MoveAction` 也可以） |
| 3. 工厂 | 加场景工厂：载荷 + `ActionTiming` + `ScheduledAction` + `ActionOf(actor)` |
| 4. 执行器 | 加执行器：`due(now)` → 落地效果 → 销毁行动实体 → 槽置 `Executing { until }` |
| 5. 把意图变成行动 | 在"开闸物化"里把 `AbilityId` 映射到第 3 步的工厂 |

**时间线一行都不用改**——这正是"调度器不感知载荷"的意思。
