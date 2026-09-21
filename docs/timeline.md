# 时间线：决策槽 · 行动实体 · 世界冻结

> ⚠️ **目标设计**：标 🚧 的部分代码里还没有；**决策槽换形状已落地**（M23，见第一节）。
> 第九节记录了这次改动的理由与代价。

三个概念各管一件事，互不代替：

| 概念 | 管什么 | 住在哪 |
| :--- | :--- | :--- |
| **决策槽** | 这个单位此刻能不能决策、决定了没有 | 单位身上 |
| **行动实体** | 这一手什么时候落地 | 独立实体，挂 `ActionOf(行动者)` |
| **时钟** | 世界现在停不停 | `Time<Virtual>`，只由 `apply_clock` 写 |

## 一、决策槽 ✅

```rust
pub enum DecisionSlot {
    /// 空闲：`intent` 空 = 还没决定；有 = 已决定、但这一手还没排进时间轴
    Idle { intent: Option<Intent> },
    /// 这一手已经在时间轴上（前摇 → 执行 → 后摇），`until` = 忙到什么时候
    Executing { until: f32 },
}
```

**槽里装的是"决策"，不是"阶段"**。这一手执行到哪一步由行动实体自己的
`ScheduledAction.execute_at` 回答（`now < execute_at` 前摇 / 过了该执行），
槽不重复表达一遍。

判据只有一个：

```rust
impl DecisionSlot {
    /// 这个单位此刻算不算「已经决定了」（暂停断言只看这一条）
    fn ready(&self) -> bool {
        match self {
            DecisionSlot::Executing { .. } => true,
            DecisionSlot::Idle { intent } => intent.is_some(),
        }
    }
}
```

## 二、意图 ✅

```rust
/// 一次「已决定、还没排进时间轴」的决策。
pub struct Intent {
    pub ability: AbilityId,   // 技能 / 移动 / 跳跃 / 翻滚 / 招架…（见 skills.md）
    pub target: Target,
}

pub enum Target {
    None,                 // 不需要目标（跳跃、招架自己找威胁）
    Cell(Cell),           // 锁一格（火球落点、移动目标格）
    Entity(Entity),       // 单体目标（招架绑定的那次攻击）
}
```

**意图只描述"想做什么"，不做任何落地**：不扣资源、不生成实体、不改世界。
填意图之前跑一次 [`can_cast`](skills.md)（条件够不够）🚧，被拒就写一条
`ActionBlocked` 给 HUD —— 和玩家按键失败的手感是同一条路
（`ActionBlocked` 与"各声明系统自己查条件"已落地，`can_cast` 这个统一入口还没有）。

> **名字的层级**（三层，不要混）：`ai::Situation`（读到的战况）→ `ai::Tactic`
> （战术选择：靠近 / 贪刀 / 闪避，**已落地**）→ `Intent`（**动作决策**，已落地但
> **暂时没有读者**：声明即物化，意图在同一帧就被行动实体取代）。
> 玩家的 `PlayerTakeover` 是"我要改主意"这一条**输入层事实**，不是一个意图
> （**已落地**：`input` / `interaction` 写、`timeline::undo_system` 消费）。

## 三、一轮里发生什么

没有轮次、没有"提交"动作，但**一帧之内的顺序是语义的一部分**——世界冻结时按这个
顺序走：

```text
① 非 PC 的决策        所有空着的决策槽 → can_cast 🚧 → 填 Intent 🚧
                      → 立刻物化成行动实体（execute_at = 现在 + 自己的前摇）。
                      AI 在这一步决策。
② 威胁扫描            前摇中、玩家还没表态的行动 → 与 PC 所在格相交 → 记入 ReactionSlot
③ PC 的决策           槽空                    → 断言 Pause("awaiting")
                      有威胁 且 ReactionSlot 空 → 断言 Pause("threat")
                      同时高亮「有反制资源可用」的技能
④ 结算（时间流动时）   到点 → 效果判定：命中 / 防御链 / 打断 / 扣血（combat.md 第一节）
```

关键点：

- **没有独立的"闸门"概念**。能不能决策由**决策槽**回答（③ 的暂停断言），
  效果能不能落地由**效果判定**回答（④）——两者都不需要一个额外的门控机制。
- **"物化"是各域自己的事**（只有它认识自己的载荷），时间线**不认识载荷**；
  谁声明谁物化：`movement` 物化移动，`combat` 物化攻击，**没有全局派发器**
  （`AbilityId` 只是静态目录的键，见 [skills.md](skills.md)）。
- **① 一定发生在 ③ 之前**：敌人不会因为"还没想好"让玩家多等一帧，
  所以暂停实际上只等 PC。
- **后摇截止时刻存在槽里**（`Executing { until }`）：行动实体在执行时就销毁了，
  "忙到什么时候"必须另找地方放。
- **槽回 `Idle` 时广播 `DecisionReady`**（`EntityEvent`），关心它的域自己订阅
  （目前是 `combat::defense` 回 1 点精力）——时间线不反向依赖资源。

### 为什么"同时提交"是免费的

冻结时 `Time<Virtual>` 不走。① 和 ③ 都发生在**同一个冻结时刻**，物化出来的行动
`execute_at = 同一时刻 + 各自前摇`——**双方起跑线相同，和你思考了 5 秒还是 0.5 秒无关**。
这正是战术暂停想要的"决策同时提交、按各自前摇排序执行"，不需要额外的"提交"动作、
也不需要 ActionQueue。

## 四、冻结：原因集合

冻结的判据只有一条：**`PauseReasons` 非空**（见第五节）。原因里没有"闸门"这一项，
只有 ③ 那两条断言与玩家的手动暂停：

```text
PC 的决策槽还空着（还没决定）  → 断言 Pause(SLOT_EMPTY)   // 语义 = "awaiting"
PC 正被威胁、反应槽还空着      → 断言 Pause(THREAT)
玩家按了暂停键                → 输入域发 Toggle(MANUAL)（翻转冻结状态）
```

敌人"无感"：AI 在 ① 就把意图填好，所以 `"awaiting"` 实际上只等玩家。

## 五、时钟

```text
frozen ⟺ 本帧的 PauseReasons 非空
```

- 请求有**两种时序**，别混：
  - `PauseRequest::Pause(reason)` 是**断言式**：谁这一帧还想让世界停着就写一条；
    下一帧不再断言，原因自然消失，**不需要谁去撤销**（各领域走这条）。
  - `PauseRequest::Toggle(reason)` 是**翻转**：冻着就**清空原因集合**（世界立刻动）、
    没冻就停住并把原因闩进 `timeline::LatchedReasons`（此后每帧自己续上）。
    输入域只发一条 `Toggle`，**不读** `PauseReasons`。
- **为什么分成两种**：断言式原因每帧都要重新声明，而玩家按键是**一次性事件**。
  共用一条消息就得先猜"上一帧有没有人断言过这个原因"——而集合里同时躺着别人的
  原因，猜不准。曾经的 bug 就是从 `PauseReasons` 反推手动暂停：威胁冻着时集合里
  只有 `"threat"`，于是把"继续"误判成"暂停"，空格按下去反而又停一层。
- **落地顺序是语义的一部分**：`Toggle` 由 `apply_pause_toggles_system` 在
  `TimelineSet` **最前面**落地（早于各领域的断言），否则同一帧里"威胁还在断言
  `Pause(THREAT)`"会把玩家刚清掉的原因加回来。各领域的断言在帧末
  `process_pause_requests` 统一收进集合（手动开关 + 本帧断言）。
  顺序确定：输入域先、各域断言后——**同一帧里仍然成立的原因会照常加回来**
  （比如"还等着你决策"），所以"放开世界"不等于"跳过决策"。

当前的原因：`"manual"`（玩家翻开的开关）/ **`"awaiting"`**（PC 还没决定）
/ `"threat"`（PC 被威胁且反应槽空着）。

> 命名差异：`"awaiting"` 是**当前代码里的常量名**（`timeline::SLOT_EMPTY`），
> 本文其余地方按语义写作 `"awaiting"` 🚧——改名是后续项，不是现状。
> 引用常量一律用 `SLOT_EMPTY` / `MANUAL` / `THREAT`，**不要硬编码字符串**。

**唯一**写 `Time<Virtual>` 的地方是帧末 `ClockSet` 里的 `apply_clock`。Bevy 每帧把虚拟
时间拷进通用 `Time`，因此位移、投射物、`Lifetime`、后摇计时全部自动停表，
各域不需要任何 `if paused` 分支。

## 六、行动实体

行动 = **独立实体**，身上只有几样（谁能撤它、它的节奏、它什么时候落地），
归属靠关系：

| 组件 | 挂哪 | 回答 |
| :--- | :--- | :--- |
| 载荷（`MoveAction` / `FireballAction` / `MeleeAction`…） | 行动实体 | 这一手是什么 |
| [`ActionTiming`](skills.md)（前摇 / 后摇 / 打断抗性） | 行动实体 | 这类动作的节奏（**值归各域**） |
| `ScheduledAction { execute_at }` | 行动实体 | 这一手什么时候落地 |
| `Uncancellable` | 行动实体 | 不许撤（跳跃那种"起跳不插队"） |
| `ActionOf(actor)` / `Actions` | 行动 ↔ 行动者 | 这一手是谁的 |

**归属用自定义关系而不是 `ChildOf`**：行动实体没有 `Transform`，归属是纯逻辑；
`Actions` 标了 `linked_spawn`，因此"行动者阵亡 / 重置 → 名下行动跟着销毁"这条
结构保证照样成立（详见 [relations.md](relations.md)）。

## 七、撤销 与 打断

| | 判据 | 结果 |
| :--- | :--- | :--- |
| **撤销**（右键 `UndoCommand` / 换手 `PlayerTakeover`） | 行动实体还在、`pending(now)`（还没到点）、没挂 `Uncancellable`、行动者是玩家 | trigger `ActionCancelled` → 销毁行动实体 → 槽回 `Idle` → **退款由花钱的域订阅** |
| **打断**（被打中） | 命中 + [标签](combat.md)允许 + 掷骰对抗赢了 | 销毁行动实体 → 槽回 `Idle`（**不退款**：那一手白费了） |

两者都不看"槽是不是 `Windup`"——**阶段由行动实体自己回答**，槽只回答
"有没有决策 / 在不在执行"。

## 八、坐标：决策按格、结算按真实距离

| | 决策层 | 结算层 |
| :--- | :--- | :--- |
| 类型 | `Cell { x, z }` | `Transform.translation`（世界单位） |
| 单位 | 格（`CELL_SIZE = 2.0`，定义在 `movement`） | 世界单位 |
| 管什么 | 谁能决策、走哪一格、锁哪一格、威胁哪几格 | 命中、射程、爆炸半径、位移 |
| 谁更新 | `move_entities_system` 只在**停下**时写 | 每帧由 `Velocity` 推进 |

换算只有一个入口：`Cell::center()` / `Cell::from_world()`。命中 / 射程 / 爆炸一律
用真实距离。

## 九、与"阶段式决策槽"的差异（为什么改）

旧模型把**阶段**放进槽（`Empty` / `Windup` / `Recovery { until }`），新模型放**意图**：

| | 旧：阶段式 | 新：意图式 |
| :--- | :--- | :--- |
| 槽里装什么 | 这一手到哪一步了 | 决定了没有 / 在不在执行 |
| 决策怎么进时间轴 | 声明即排期 | 声明 = 填意图 + 当场物化（第三节 ①） |
| 前摇 / 后摇 | 槽的两个状态 | 行动实体的 `execute_at` + 槽的 `until` |
| 撤销 / 打断的判据 | 槽是 `Windup` | 行动实体在且 `pending` |
| "谁在写槽"的锚点 | `insert(DecisionSlot::Windup)` | 填意图那一处 |
| `can_cast`（条件校验） | 散布在各声明系统 | **填意图之前**统一跑一次 |

**行为上几乎等价**（因为冻结时虚拟时间不走，双方的 `execute_at` 天然同基准，
见第三节）——换的是**数据模型与语义**：槽回答"决定了没有"，阶段由行动实体回答；
顺带给"释放条件"（[skills.md](skills.md) 的 `can_cast`）找到了唯一的落点。
