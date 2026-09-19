# 战斗：命中管线 · 对抗 · 伤害

> ⚠️ **目标设计**：标 🚧 的部分代码里还没有（现状是"闪避 → 招架 → 护甲 → 扣血"，
> 打断用掷骰、无格挡、无标签）。

本域回答三个问题：**谁打中了谁**、**扣多少**、**能不能把对方那一手打掉**。

## 一、命中管线

一次命中按顺序过这几关，**前面拦下了就不走后面**：

```text
目标获取（形状相交 / 扇形）
  └─▶ ① 闪避（Dodging）      : 拦下 → 伤害归零
  └─▶ ② 招架（Parrying）     : 拦下 → 伤害归零 + 反制一半
  └─▶ ③ 格挡（Blocking）🚧   : 拦下 → 按格挡率减伤（不是归零）
  └─▶ ④ 抗性减伤             : 按伤害类型各减各的
  └─▶ ⑤ 扣血                 : 唯一的扣血点，广播 DamageEvent
  └─▶ ⑥ 触发打断 / 施加 debuff
```

**"有没有吃到冲击"决定第 ⑥ 步**：被闪开 / 被招架 = 没吃到冲击，因此**不触发打断**
（挡住一次攻击不该反被打断）。格挡是**减伤不是免伤**，所以照样触发打断。

管线只有一份实现：`combat::formula` 的命中系统逐个伤害类型各一个（形状相同），
判定逻辑进 `formula/domain.rs`（**纯函数、零 Bevy、可单测**）。

## 二、对抗标签 🚧

"这一手能不能被反制"是**设计事实**，写在技能定义上（见 [skills.md](skills.md)）：

```rust
pub struct CombatTags {
    pub interruptible: bool,   // 前摇中能不能被打断
    pub super_armor: bool,     // 霸体：豁免打断
    pub parryable: bool,       // 能不能被招架
    pub blockable: bool,       // 能不能被格挡
}
```

**标签做闸门、掷骰做对抗**，各管一件事：

| | 谁决定 | 怎么判 |
| :--- | :--- | :--- |
| 能不能打断 | 标签 | `interruptible && !super_armor`，否则连对抗都不做 |
| 这一次断不断 | 掷骰 | `interrupt_lands(power, resist, 攻骰, 守骰)`（纯函数） |
| 能不能招架 / 格挡 | 标签 | `parryable` / `blockable` |

掷骰给的是**不确定性**（玩家可以赌一把），标签给的是**设计可控**（霸体 Boss 打断不了）。

打断的落地：销毁目标那条**还没到点**的行动实体 → 决策槽回 `Idle`。
**不退款**——那一手白费了。

## 三、伤害与抗性

**一种伤害 = 一个组件**，一种抗性 = 一个组件，**没有中心化的 `DamageType` 枚举**：

| 输出侧（攻击实体） | 防御侧（单位） |
| :--- | :--- |
| `PhysicalDamage(i32)` 🚧 已有 | `PhysicalResist(f32)` 🚧（现在是 `Armor(i32)`） |
| `FireDamage(i32)` 🚧 | `FireResist(f32)` 🚧 |
| `IceDamage(i32)` 🚧 | `IceResist(f32)` 🚧 |

加一种伤害 = 加两个组件 + 一个与既有命中系统同形的系统 + 在场景工厂挂上。
生命 / 死亡 / 日志 / 撤销都不需要知道新类型存在。

**持续效果**也是独立组件，各自一个 tick 系统：`Burn` / `Slow` / `Poison` 🚧。

## 四、威胁与反应槽 🚧

威胁 = "有敌对的东西正打在玩家头上"。它触发**自动暂停**，并给玩家一个反应窗口。

```rust
/// 一次威胁开一个窗口（挂在**被威胁的玩家**身上）
pub struct ReactionSlot {
    pub threat: Entity,               // 是哪条行动 / 哪颗投射物
    pub options: Vec<AbilityId>,      // 可以拿哪几手反制（由标签匹配算出来）
    pub resolved: bool,               // 玩家表态了没有
}
```

流程：

```text
敌对行动 / 投射物瞄准玩家所在格
  └─▶ 断言 Pause("threat")           （每帧断言，见 timeline.md 第五节）
  └─▶ 插入 ReactionSlot + 算出可反制项（CounterSuggestion → HUD 高亮）
  └─▶ 玩家选一个反制 / 放弃
  └─▶ 按 CounterCost 改时间轴，resolved = true
  └─▶ 窗口关闭；威胁消失时不再断言，世界解冻
```

**反制是独立资源，不覆盖决策槽**——反应槽与决策槽共存，互不改结构。

```rust
pub enum CounterCost {
    Free,                  // 反制插入，原决策保留
    ActionPoint(u32),      // 消耗资源，原决策保留
    CancelDecision,        // 从时间轴移除原决策，插入反制
}
```

反制插入时间轴的位置：`elapsed + 一小段`（紧接当前时刻），而不是排到队尾。

**多威胁一次只处理一个**，处理完再检测下一个。

> 这一节取代当前的 `ThreatWindow` 资源。旧实现靠"玩家那一手变了没有"推断表态，
> 玩家在**前摇中**被冻结时换手也等不到表态，双方会永久冻死；新模型直接存
> `threat: Entity` + `resolved: bool`，没有这个洞。

## 五、单位身上的战斗资源

| 组件 | 说明 |
| :--- | :--- |
| `Health { current, max }` | 唯一的生命真相 |
| `Stamina { current, max }` | 动作消耗；**后摇结束时回 1 点**（订阅 `DecisionReady`） |
| `Focus { current, max }` 🚧 | 反应资源：1 点把一次声明的前摇归零；每 10 虚拟秒回 1 点 |
| `Faction { Player \| Enemy }` | **只管战斗目标过滤**，不代表"谁在操作"（那是 `InputDriven`） |

资源归**拥有它的域**：谁能拿到它（回、扣）由该域的 Observer 决定，
时间线只宣布"后摇结束了"（`DecisionReady`）。
