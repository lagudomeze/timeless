# 域地图 与 跨域契约

> ⚠️ **目标设计**：标 🚧 的部分代码里还没有。当前与本文的差异有三处：
>
> 1. `combat` 还是**一个** `CombatPlugin` 装着 7 个子域（本文要求每个 mod 出自己的
>    `plugin.rs`，父域只编排）——**7 个子域目前 0 个 `plugin.rs`**；
> 2. 攻击子域目录仍叫 `combat/skills`（本文按语义写作 `combat::attack`）；
> 3. `utils` 域**还没建**（`Shape` 一族在代码里不存在，见第一节末）。
>
> 落地进度见 `TODO.md` M22+。

> 本篇回答三件事：**有哪些域**、**域之间怎么说话**、**什么算违规**。
> 铁律在文末；每个域的内部设计见各自的专题篇。

## 一、一个域 = 一个目录 = 一个 `Plugin`

顶层域（`src/` 下的一级目录）：

| 域 | 一句话职责 | 谁依赖它 |
| :--- | :--- | :--- |
| `world` | 体素地图**数据**：区块、地形生成、体素存取（零渲染依赖） | `voxel_render`、`movement`、`spawn` |
| `voxel_render` | 体素**表现**：异步网格化、材质、明暗 | —（消费 `world` 的区块消息） |
| `movement` | 格子坐标（`Cell`）+ 连续位移（`Velocity`）+ 移动 / 跳跃 / 翻滚载荷与执行器 | `combat`、`ai`、`interaction`、`spawn` |
| `combat` | 战斗的**全部子域**（见下） | `ai`、`spawn`、`presentation` |
| `skills` | 技能**静态定义**：`AbilityId` / `AbilityDef` / `Requirement` / `can_cast` / 反制代价；**移动 / 跳跃 / 翻滚也是技能**，不做特殊处理 | 几乎所有域（读目录） |
| `timeline` | 决策槽 + 行动实体 + 世界何时冻结 | 几乎所有域 |
| `ai` | 敌人决策（填意图） | — |
| `input` | 键盘 / 鼠标 → **消息**（只翻译） | —（没有域依赖它） |
| `interaction` | 鼠标拾取、高亮、点击 → 消息 | — |
| `presentation` | 相机 / 单位纸片 / HUD / 日志（**只读**） | — |
| `spawn` | **组装车间**：把各域零件拼成"玩家 / 敌人" | —（没有域依赖它，唯一例外见文末） |

### `combat` 的子域（每个都是独立的 mod）

| 子域 | 一句话职责 |
| :--- | :--- |
| `attributes` | 战斗数值属性：伤害、抗性、命中形状、打断力度 |
| `health` | 谁还有多少血、什么时候死 |
| `targeting` | 打到了谁（形状相交 / 扇形） |
| `lifecycle` | 攻击实体的存活、命中计数、清理 |
| `formula` | 命中结算：防御链 → 减伤 → 扣血 → 触发打断（纯公式住 `domain.rs`，零 Bevy） |
| `attack` 🚧 | 攻击行动：火球 / 横扫 / 箭矢 + 爆炸（载荷 + 工厂 + 执行器）——**现在这个目录叫 `skills`** |
| `defense` | 翻滚 / 招架 / 格挡 |
| `reaction` | 威胁探测 → 开反应槽 → 反制（`ReactionSlot` + `CounterSuggestion`） |

**技能定义不住在 `combat` 里**：`AbilityDef` 一族搬去顶层 `skills` 域（那是静态目录，
战斗只是最常读它的那个）；`combat` 只留"这一手怎么打出来"。谁声明谁物化——
`movement` 物化移动、`combat` 物化攻击，**没有全局派发器**。

**每个 mod 出自己的 `plugin.rs`**；父域（`CombatPlugin`）**只负责编排子域之间的顺序**，
不自己注册系统、不自己定义组件。这条对 `combat` / `voxel_render` / `world` 都成立。

### 不是领域的东西

| 名字 | 是什么 | 为什么不是领域 |
| :--- | :--- | :--- |
| `utils` 🚧 | 纯几何 / 纯类型工具（`Shape`…），**规划中**源码在 `src/utils/` | 没有数据模型、没有系统、没有 Plugin；**纯函数谁都能直接引用**（见第三节第 3 条） |

> `utils` 目前**还没有建**：`Shape` 一族不存在，几何判定分散在
> `combat/formula/domain.rs` 与 `movement/cell.rs` 里。本文把它写成契约是
> **目标状态**；落地前请勿在 `src/` 里按 `utils::` 路径引用。

## 二、域之间怎么说话

**两条通道，不可混用**（`AGENTS.md` 铁律）：

| 通道 | 什么时候用 | 形态 |
| :--- | :--- | :--- |
| `Message` | 广播、批量、可以晚一帧 | `MessageWriter` / `MessageReader` |
| `EntityEvent` + `Observer` | 定向到具体实体、必须当场生效 | `commands.trigger(..)` / `On<E>` |

**消息定义在消费它的域**（消费方注册 `add_message::<T>()`），并在文档注释里写清
「谁写、谁消费」。唯一例外是 `ResetBattle`：它定义在 `spawn`（消费方），由 `input` 写。

### 跨域消息清单

| 消息 | 谁写 | 谁消费 |
| :--- | :--- | :--- |
| `MoveCommand` / `MoveToCommand` / `JumpCommand` | `input` / `interaction` | `movement` 的声明系统 |
| `FireCommand` / `MeleeCommand` | `input` / `combat::attack` 的菜单派发 | `combat::attack` 的声明系统 |
| `RollCommand` / `ParryCommand` | `input` | `combat::defense` 的声明系统 |
| `SelectSkill` / `CycleSkill` / `UseSelectedSkill` | `input` | `combat::attack` 的菜单 |
| `RegisterAbility` | 各机制域（`movement` / `combat`） | `skills` 的注册表 |
| `CounterCommand` / `AbandonReaction` 🚧 | `input` | `combat::reaction` |
| `PauseRequest` | `input`（手动）/ `timeline`（等 PC 决策）/ `combat::reaction`（威胁） | `timeline::process_pause_requests` |
| `PlayerTakeover` | `input`（键盘）/ `interaction`（左键） | `timeline::undo_system` |
| `UseFocus` | `input` | `timeline` |
| `UndoCommand` | `interaction`（右键） | `timeline::undo_system` |
| `ActionBlocked` | 各声明系统 / `skills::can_cast` 的失败 | `presentation` 的提示条 |
| `ActionCancelled` | `timeline::undo_system` | 花钱的域（`skills` 退款） |
| `DecisionReady` | `timeline::recovery_system` | `combat::defense`（回精力） |
| `DamageEvent` / `DeathEvent` | `combat::formula` / `combat::health` | `combat::health` / `presentation` 的日志 |
| `ProjectileArrived` | `combat::attack`（火球到达） | `combat::attack`（爆炸） |
| `PointerCommand` | `input` | `interaction`（解释成走 / 打 / 撤） |
| `PanCamera` / `ZoomCamera` / `ToggleHelp` / `PreviewReadout` | `input` / `interaction` | `presentation` |
| `ResetBattle` | `input`（`F5`） | `spawn` |
| `ChunkLoadEvent` / `ChunkUnloadEvent` / `ChunkDirtyEvent` | `world` | `voxel_render` |

### 跨域事件（`EntityEvent` + Observer）

| 事件 | 谁触发 | 谁订阅 |
| :--- | :--- | :--- |
| `InterruptEvent`（住 `combat::formula`） | 命中系统 | `combat::formula::interrupt_observer` |
| `ActionCancelled`（住 `timeline`） | `timeline::undo_system` | 各域的退款 Observer |
| `DecisionReady`（住 `timeline`） | `timeline::recovery_system` | `combat::defense::recover_stamina_observer` |

## 三、import 规则：什么能直接引用，什么必须走消息

规则只有一条分界：**数据可以共享，行为必须解耦。**

| 类别 | 能否跨域直接引用 | 说明 |
| :--- | :--- | :--- |
| **组件类型**（`Health` / `Cell` / `DecisionSlot` …） | ✅ 可以读 | 组件是**数据契约**；写者仍然唯一（见铁律） |
| **纯类型 / 常量**（`ActionTiming`、`CELL_SIZE`、`*_TIMING`、`utils::Shape` 🚧） | ✅ | 没有行为 |
| **纯函数**（`combat::formula::domain::*`、`utils::*` 🚧） | ✅ | 零 Bevy、可单测 |
| **别人的系统** | ❌ 禁止调用 | 排顺序用 `SystemSet`，不要 `run_system` 互调 |
| **别人的内部状态**（别人的 `Resource`、`Local`） | ❌ | 要么它写成消息/事件，要么它自己算 |
| **一次操作的请求 / 结果** | ❌ 必须走消息或事件 | 例：撤销退款走 `ActionCancelled`，不是 `defense` 直接改别的域的账 |

**判定口诀**：我要的是**它身上的数据**（读组件）→ 直接引用；我要的是**它做一件事**
（请求 / 通知）→ 发消息或事件。

## 四、执行顺序

跨域顺序只在 `lib.rs::configure_pipeline` 里声明一次；域内部的子域顺序由各自的
`plugin.rs` 维护（测试复用同一入口，跑的就是真实流水线）。

```text
Startup:  PreloadSet ─▶ AssemblySet
Update:   SpawnSet ─▶ InputSet ─▶ InteractionSet ─▶ TimelineSet ─▶ AiSet
          ─▶ MovementSet ─▶ CombatSet ─▶ VoxelRenderSet ─▶ PresentationSet ─▶ ClockSet
WorldSet ──────────────────────▶（必须早于 VoxelRenderSet）
ClockSet 排在帧末：这一帧所有系统看到同一个冻结状态，唯一的时钟写入点在这里
```

**这个顺序不是性能选择，是语义的一部分**（[timeline.md](timeline.md) 第三节的
"一轮里发生什么"）：一帧按 `输入 → AI 决策 → 执行器 / 威胁扫描 → 帧末暂停落地` 走。

- `InputSet` 在 `AiSet` 之前：玩家这一帧的表态先落地。
- `AiSet` 在 `CombatSet` 之前：**敌人先决策**，威胁扫描（`combat::reaction`）才能
  看到它刚生成的前摇行动——否则永远扫不到东西。
- 暂停断言由**各域自己写**（`timeline` 写 `"awaiting"`、`combat::reaction` 写
  `"threat"`、`input` 写 `"manual"`），统一在帧末 `ClockSet` 生效——
  所以谁写在哪一段都不影响"这一帧的结论一致"。

## 五、铁律

1. **纯数据域**：`world` 不引用渲染类型，两域只经区块消息通信。
2. **UI 输入只翻译、不执行**：键盘 / 鼠标只写消息；哪个键做什么属于 `input` 域，
   其它域不认识 `KeyCode`。
3. **组件写入者唯一**：一个组件只有一处（或一族明确列举的系统）能写。
   例：`DecisionSlot` 只有声明 / 开闸 / 撤销 / 后摇恢复这几处写。
4. **消息定义在消费方**，并在注释里写清「谁写、谁消费」。
5. **表现层只读**：`presentation` 不写游戏状态。
6. **角色实体不是模块**：零件归各域，组装归 `spawn`；**没有任何域依赖 `spawn` 的
   组装逻辑**（唯一例外是 `input` 写 `spawn::ResetBattle` 这一条消息）。
7. **物理附着用 `ChildOf`，逻辑关系用自定义关系**（见 [relations.md](relations.md)）。
8. **行动实体化**：行动 = 独立实体（载荷 + `ScheduledAction` + `ActionTiming`），
   归属用 `ActionOf` / `Actions`，调度器不感知载荷。
9. **唯一的暂停判据是 `PauseReasons` 非空**，且各域用**每帧断言**加减原因；
   只有帧末 `ClockSet` 的 `apply_clock` 能写 `Time<Virtual>`。
10. **执行器自己收尾**：到点落地 → 销毁行动实体 → 把行动者推进后摇。
    没有集中式收尾函数。
11. **领域层零 Bevy**：`combat/formula/domain.rs` 可脱离 App 单测；应用层不写公式。
12. **文档防漂移**：文档里引用的类型名必须先在 `src/` 里 grep 确认存在；
    还没落地的（`can_cast` / `Requirement` / `utils` / `combat::attack`）**必须标 🚧**。
