# 战斗：命中管线 · 对抗 · 伤害

> ⚠️ **目标设计**：**命中管线六关全部落地**（含格挡 ③），对抗标签 ✅（第二节），
> 反应槽 / 反制 ✅（第四节）。仍未落地的是**伤害类型**（只有物理）、
> **持续效果**（`Burn` / `Slow` / `Poison`）与 `PhysicalResist(f32)`（现在还是 `Armor(i32)`）。
> 已经落地的部分标 ✅，引用前先 `grep` 确认。

本域回答三个问题：**谁打中了谁**、**扣多少**、**能不能把对方那一手打掉**。

## 一、命中管线

一次命中按顺序过这几关，**前面拦下了就不走后面**：

```text
目标获取（形状相交 / 扇形）
  └─▶ ① 闪避（Dodging）      : 拦下 → 伤害归零
  └─▶ ② 招架（Parrying）     : 拦下 → 伤害归零 + 反制一半
  └─▶ ③ 格挡（BlockChance）  : 拦下 → 按格挡率减伤（不是归零）
  └─▶ ④ 抗性减伤             : 按伤害类型各减各的
  └─▶ ⑤ 扣血                 : 唯一的扣血点，广播 DamageEvent
  └─▶ ⑥ 触发打断 / 施加 debuff
```

格挡率的**基础值**在单位身上（`BlockChance`），**来源是装备**（M27 已落地的
`equipment` 域：一面盾给 `+0.35`）——命中管线读的是"基础 + 装备加成"的有效值，
它只读、不自己算。

**"有没有吃到冲击"决定第 ⑥ 步**：被闪开 / 被招架 = 没吃到冲击，因此**不触发打断**
（挡住一次攻击不该反被打断）。格挡是**减伤不是免伤**，所以照样触发打断。

管线只有一份实现：`combat::formula` 的命中系统逐个伤害类型各一个（形状相同），
判定逻辑进 `formula/domain.rs`（**纯函数、零 Bevy、可单测**）。

## 二、对抗标签 ✅

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
| 能不能打断 | 标签 | `interruptible && !super_armor`，否则连对抗都不做 ✅ |
| 这一次断不断 | 掷骰 | `interrupt_lands(power, resist, 攻骰, 守骰)`（纯函数） |
| 能不能招架 / 格挡 | 标签 | `parryable` / `blockable` |

掷骰给的是**不确定性**（玩家可以赌一把），标签给的是**设计可控**（霸体 Boss 打断不了）。

打断的落地：销毁目标那条**还没到点**的行动实体 → 决策槽回 `Idle`。
**不退款**——那一手白费了。

## 三、伤害与抗性

**一种伤害 = 一个组件**，一种抗性 = 一个组件，**没有中心化的 `DamageType` 枚举**：

| 输出侧（攻击实体） | 防御侧（单位） |
| :--- | :--- |
| `PhysicalDamage(i32)` ✅ | `Armor(i32)` ✅（`PhysicalResist(f32)` 🚧 是目标形态） |
| `FireDamage(i32)` 🚧 | `FireResist(f32)` 🚧 |
| `IceDamage(i32)` 🚧 | `IceResist(f32)` 🚧 |

加一种伤害 = 加两个组件 + 一个与既有命中系统同形的系统 + 在场景工厂挂上。
生命 / 死亡 / 日志 / 撤销都不需要知道新类型存在。

**持续效果**也是独立组件，各自一个 tick 系统：`Burn` / `Slow` / `Poison` 🚧。

## 四、威胁与反应槽 ✅

> **已落地**（M26）。反应窗口是挂在**被威胁的玩家**身上的 `ReactionSlot`，
> 建议列表**从目录算出来**（遍历 `counter != None` 的技能），没有任何硬编码白名单。
> 旧实现（`ThreatWindow` 资源 + 靠"那一手变了没有"推断表态）已删除。
威胁 = "有敌对的东西正打在玩家头上"。它触发**自动暂停**，并给玩家一个反应窗口。

```rust
/// 一次威胁开一个窗口（挂在**被威胁的玩家**身上）
pub struct ReactionSlot {
    pub threat: Entity,                       // 是哪条行动 / 哪颗投射物
    pub suggestions: Vec<CounterSuggestion>,  // 能拿哪几手反制（见下）
    pub resolved: bool,                       // 玩家表态了没有
}

/// 一条反制建议 = **一个技能 + 它作为反制要付的代价**。
pub struct CounterSuggestion {
    pub ability: AbilityId,
    pub cost: CounterCost,   // 技能的静态属性，见 skills.md 第四节
    pub affordable: bool,    // 现在付得起吗（HUD 决定亮不亮）
}
```

`suggestions` 的算法只有一条：**遍历注册表里所有 `counter != None` 的技能**
（[skills.md](skills.md)），先过 `can_cast`（这一手此刻能不能出手），再算
`affordable`。**没有任何硬编码的反制列表**——"翻滚能躲火球"是翻滚自己的
`counter` 字段说的，不是这里判的。

### 一轮里的三步（与 [timeline.md](timeline.md) 第三节对应）

```text
① 扫描        所有「前摇中、玩家还没表态」的行动，与 PC 所在格相交 → 有威胁
② 开窗口      取**最先落地**的那一个当 `threat`（多威胁一次只处理一个），
              算 suggestions → 断言 Pause("threat")
③ 玩家表态    按技能键 → 用这一手反制；按右键 → 放弃这一轮反制（威胁照常落地）
```

- **暂停断言 = `威胁存在 && !resolved`**：窗口一开就冻住，玩家不表态就不解冻。
  （口径里的"`ReactionSlot` 空"按这个实现：**"空"= 还没有一个已表态的窗口"**，
  否则窗口一开世界就解冻、玩家反而没机会点。）
- **退出只有两条**：表态，或者威胁自己消失（被打断 / 已经落地 / 投射物没了）。
  没有第三条——玩家什么都不做时世界**一直冻着**，这是刻意的：战术暂停里
  "我在想" 必须能无限期地想下去。

### 反制的落地

```rust
pub enum CounterCost {
    Free,                  // 白送：反制插入，原决策保留
    Resource(u32),         // 花反制资源（当前是 Focus），原决策保留
    CancelDecision,        // 从时间轴移除原决策，插入反制
}
```

- 反制**是独立资源、不覆盖决策槽**——反应槽与决策槽共存，互不改结构。
- 反制行动插入时间轴的位置：`elapsed + 一小段`（紧接当前时刻），不排到队尾
  ——反制要"来得及"才有意义。
- 付不起的那一条**仍然列出来**（`affordable: false`），只是 HUD 画成不可选：
  玩家看得见"我本来能用招架，但精力不够"，这比看不见更有信息量。

### HUD 怎么表现（`presentation` 只读）

| 位置 | 画什么 | 落地 |
| :--- | :--- | :--- |
| 技能栏 | 有 `suggestions` 时**高亮**其中 `affordable == true` 的技能；`affordable == false` 的压暗并标注代价 | ✅ `hud::skills::model::counter_hints` → `slot_bg` / `slot_border`（反制色压过"选中"色） |
| 顶部时间轴 | 高亮 `threat` 那一条（玩家能看出"打过来的是它"） | 🚧 |
| 提示条 | 一行文案：反制可选 / 右键放弃 | 🚧 |

HUD 只读 `ReactionSlot`，不认识 `CounterCost` 的语义——它只画
"亮 / 不亮、代价是多少"。**输入只翻译**：技能键 → `ReactionAnswer::Counter(ability)` ✅，
右键 → `ReactionAnswer::Abandon` ✅；两者都由 `combat::reaction` 消费
（右键本来就同时写 `UndoCommand`，没有窗口时 `AbandonReaction` 自然被忽略，
输入域因此不需要去读游戏状态）。

> **旧实现已删除**：`ThreatWindow` 靠"玩家那一手变了没有"推断表态，
> 而那个判据在**后摇 / 不可撤行动**期间永远为假（没槽可声明、也没行动可撤），
> 会把玩家锁死。新模型直接存 `threat: Entity` + `resolved: bool`——
> 表态是**显式的**一条消息，与玩家处于哪个阶段无关。

## 五、单位身上的战斗资源

| 组件 | 说明 |
| :--- | :--- |
| `Health { current, max }` | 唯一的生命真相 |
| `Stamina { current, max }` | 动作消耗；**后摇结束时回 1 点**（订阅 `DecisionReady`） |
| `Focus { current, max }` | **反制资源** ✅：1 点把一次声明的前摇归零（`FOCUS_MAX = 3`，每 `FOCUS_RECOVER_INTERVAL = 10.0` 虚拟秒回 1 点）。付 `CounterCost::Resource` 🚧 还没落地 |
| `BlockChance(f32)` ✅ | 格挡率**基础值**；有效值 = 基础 + 装备加成（`equipment` 域，M27 已落地）。管线第 ③ 关读**有效值** |
| `Faction { Player \| Enemy }` | **只管战斗目标过滤**，不代表"谁在操作"（那是 `InputDriven`） |

资源归**拥有它的域**：谁能拿到它（回、扣）由该域的 Observer 决定，
时间线只宣布"后摇结束了"（`DecisionReady`）。

⚠️ **`Focus` 现在住在 `timeline/focus.rs`，要搬来这里**——它是反制资源，
不是时间线的概念（`TODO.md` 的 T4）。
