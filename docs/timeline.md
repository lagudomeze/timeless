# 时间线：决策槽 · 行动实体 · 世界冻结

> ⚠️ **目标设计**：本篇描述**接下来要落地的模型**，其中标 🚧 的部分代码里还没有
> （落地进度见 [`TODO.md`](../TODO.md)；`src/` 现状仍用阶段式决策槽
> `Empty / Windup / Recovery`）。第九节记录了这次改动的理由与代价。

三个概念各管一件事，互不代替：

| 概念 | 管什么 | 住在哪 |
| :--- | :--- | :--- |
| **决策槽** | 这个单位此刻能不能决策、决定了没有 | 单位身上 |
| **行动实体** | 这一手什么时候落地 | 独立实体，挂 `ActionOf(行动者)` |
| **时钟** | 世界现在停不停 | `Time<Virtual>`，只由 `apply_clock` 写 |

## 一、决策槽 🚧

```rust
pub enum DecisionSlot {
    /// 空闲：`intent` 空 = 等它决策；有 = 已决定、在等其他人（闸门）
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
    /// 这个单位此刻算不算「已经决定了」（闸门只看这一条）
    fn ready(&self) -> bool {
        match self {
            DecisionSlot::Executing { .. } => true,
            DecisionSlot::Idle { intent } => intent.is_some(),
        }
    }
}
```

## 二、意图 🚧

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
填意图之前跑一次 [`can_cast`](skills.md)（条件够不够），被拒就写一条
`ActionBlocked` 给 HUD —— 和玩家按键失败的手感是同一条路。

## 三、一批的生命周期

```text
① 填意图    各域声明：把 Intent 放进自己单位的槽（Idle { intent: Some(..) }）
              —— 玩家按键、或 AI 决策
② 闸门      全员 ready 之前，世界冻着（见第四节）
③ 开闸      全员 ready → 各域把自己的意图**物化**成行动实体排进时间轴，
             槽置 Executing { until: 忙到什么时候 }
④ 时间轴    时间流动：各自按 ScheduledAction.execute_at 落地（前摇 → 执行）
⑤ 后摇      执行器收尾：行动实体销毁，until = 效果落地 + 后摇
⑥ 回到 Idle until 到点 → 槽回到 Idle { intent: None }，等下一次决策
```

关键点：

- **③ 的"物化"是各域自己的事**（只有它认识自己的载荷），时间线**不认识载荷**；
  时间线只提供闸门判据与"该开闸了"这个信号。
- **⑤ 的后摇截止时刻存在槽里**（`Executing { until }`）：行动实体在执行时就销毁了，
  "忙到什么时候"必须另找地方放。
- **⑥ 到点回 `Idle` 时广播 `DecisionReady`**（`EntityEvent`），关心它的域自己订阅
  （目前是 `combat::defense` 回 1 点精力）——时间线不反向依赖资源。

## 四、冻结：原因集合 + 闸门

冻结的判据只有一条：**`PauseReasons` 非空**（见第五节）。其中"等决策"这个原因是
**闸门**：

```text
有任何一个槽 !ready（既没意图、也没在执行） → 断言 Pause(AWAITING)
所有人要么有意图、要么正在执行              → 不再断言
```

敌人"无感"：AI 在冻结的那一帧就把意图填好，所以闸门实际上只等玩家。

### 为什么"同时提交"是免费的

冻结时 `Time<Virtual>` 不走。于是玩家和敌人在**同一个冻结时刻**填意图，物化出来的
行动 `execute_at = 同一时刻 + 各自前摇`——**双方起跑线相同，和你思考了 5 秒还是
0.5 秒无关**。这正是战术暂停想要的"决策同时提交、按各自前摇排序执行"，
不需要额外的"提交"动作、也不需要 ActionQueue。

## 五、时钟

```text
frozen ⟺ 本帧的 PauseReasons 非空
```

- 原因是**断言式**的：谁这一帧还想让世界停着就写一条 `PauseRequest::Pause(reason)`；
  下一帧不再断言，原因自然消失，**不需要谁去撤销**。
- `PauseRequest::Resume` 不带原因，表示"清空此刻已收集的原因、让时间流动"，
  只由输入域发（玩家手动解冻）。
- 集合每帧重建：`process_pause_requests` 先 `clear()`，再按**发出顺序**处理这一帧的请求。
  顺序是确定的：输入域（`Resume` 的来源）排在 `TimelineSet` 之前，各域的断言排在它之后
  ——手动解冻那一帧，仍然成立的断言会照常加回来。

当前的原因：`"manual"`（玩家按了暂停键）/ `"awaiting"`（闸门，代码里现在还叫
`"slot_empty"` 🚧）/ `"threat"`（威胁逼近）。

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
| **撤销**（右键 / 换手） | 行动实体还在、`pending(now)`（还没到点）、没挂 `Uncancellable`、行动者是玩家 | trigger `ActionCancelled` → 销毁行动实体 → 槽回 `Idle` → **退款由花钱的域订阅** |
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
| 决策怎么进时间轴 | 声明即排期 | 先收集，**开闸**时一起物化 |
| 前摇 / 后摇 | 槽的两个状态 | 行动实体的 `execute_at` + 槽的 `until` |
| 撤销 / 打断的判据 | 槽是 `Windup` | 行动实体在且 `pending` |
| "谁在写槽"的锚点 | `insert(DecisionSlot::Windup)` | 填意图 + 开闸两处 |
| `can_cast`（条件校验） | 散布在各声明系统 | **填意图之前**统一跑一次 |

**行为上几乎等价**（因为冻结时虚拟时间不走，双方的 `execute_at` 天然同基准，
见第四节）——换的是**数据模型与语义**：槽回答"决定了没有"，阶段由行动实体回答；
顺带给"释放条件"（[skills.md](skills.md) 的 `can_cast`）找到了唯一的落点。
