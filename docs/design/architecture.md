# 架构原则与分层

> **描述对象：分层原则（通用）+ 状态标注。**
> 本文的**原则**（零依赖领域层、Message 选型、数据驱动、输入只翻译）两棵树都适用，
> 也是代码 A 的现行约定；但文中标注为「B」的文件布局与现状只描述
> **代码 B（`timeless/`，已冻结、本检出跑不起来）**。
> A 的实际布局见 [app-modules.md](app-modules.md)，进度见 [../status.md](../status.md)。

## 分层

| 层 | 载体 | 职责 | 约束 |
| :--- | :--- | :--- | :--- |
| 领域层 | 纯 Rust，零 Bevy | 战斗裁决（帧 → 距离 → 破势）、防御判定、反制 | **零 Bevy 依赖**，可独立单测 |
| 应用层 | Bevy 组件 / 系统 | 组件、系统、渲染、HUD 编排 | 不含伤害公式与结算逻辑 |
| 表现层 | 应用层内 | 输入、渲染、UI | 只消费领域结果，不反推规则 |

代码 A 的领域层是 `src/combat/formula/domain.rs`（`AttackStats` / `resolve_combat` /
`resolve_attack` / `resolve_defense` / `counter_damage`，**11 个单测**）。
代码 B 的领域层是独立 crate `timeless-domain`。
**两棵树的领域层都不依赖 Bevy**，这条约束两棵树都遵守。

## 通信策略（Bevy 0.19）

- **Message**（`#[derive(Message)]` + `app.add_message::<T>()`，经 `MessageWriter` / `MessageReader`）：系统间**解耦、批量、有序**的缓冲队列；双缓冲存活 2 帧，不消费会被静默清理。
- **Event / EntityEvent + Observer**（`GlobalTrigger` / `EntityTrigger`，`On<T, B>`，`commands.trigger` / `trigger_targets`）：**即时响应、定向实体**。
- **UI 输入只翻译、不执行**：键盘 / egui 面板把操作翻译成 Message，由对应领域的
  单一职责系统消费落地；输入系统不得同时做决策与状态修改。
  代码 A 的实际输入消息清单见
  [ecs-combat-components.md](ecs-combat-components.md#五消息清单谁写--谁消费)。
- **消息与其消费系统同属一个领域**：代码 A 里是
  `MoveCommand` + `declare_move_system`（`movement/`）、
  `ActionsCommitted` + `commit_bridge_system`（`timeline/`）；
  代码 B 里是 `MoveInput` + `move_input_system`（`movement.rs`）、
  `ActionsCommitted` + `finalize_declared_actions`（`timeline.rs`）。
  其他领域需要该操作时只写消息，不重复实现。

选型规则：需要立即生效或针对具体实体 → Event + Observer；批量广播、允许晚一帧、强调解耦 → Message。`MessageWriter` 发送不会触发 Observer，两者不可混用。

> `EventReader` / `EventWriter` 在 0.19 已移除，旧代码一律迁移到上述两套体系。

## 数据驱动

- 组件只存数据；**可配置类型用 ID / usize 索引 / Handle** 引用，不用空标记组件充当身份。
- 技能、怪物、标签反应等一律走外部配置（serde + ron / Asset），应用层不硬编码数值。
  **代码 A 尚未做到**：`timeline::timing` 与 `SKILLS` 的数值仍是硬编码常量
  （见 [../status.md](../status.md) 第四节）。
- 动作定义资产化为 `ActionTemplate`：**未落地的设计稿**，见
  [../bevy/action-graph.md](../bevy/action-graph.md)。代码 A 现在只有它的最小形态
  `ActionTiming { windup, recovery }`。

## 代码组织

- `main.rs` 只负责装配 `App`；业务逻辑放 `lib.rs` 的 `GamePlugin`。
- Plugin 之间严格隔离，不互相 import；跨模块只经 Message / Event 通信。
- 测试用 `App::new()` + `MinimalPlugins` 搭最小 App：
  代码 A 的整机夹具是 `crate::test_support::headless_app()`（`MinimalPlugins` +
  `GamePlugin` + 输入/时间/场景等必要插件），**不存在** `AppPlugin` 这个类型；
  单纯数据域（`world` / `voxel_render`）各自用 `MinimalPlugins` + 自己的 Plugin。

## 应用层文件布局

**代码 A（`src/`，主线）**：一个领域 = 一个目录 = 一个 Plugin，领域内部按职责
分文件（`components` / `events` / `systems` / `resources`），完整树见
[app-modules.md](app-modules.md)。

**代码 B（`timeless/`，已冻结）**：按领域一文件组织，**不设独立的 systems 目录**
——一个领域文件里组件、消息、系统同放（高内聚）；只有 `display` 按表现子域拆成
mod 目录。下方目录树中的 `Position` / `Can*` / `Damage` 都是 **B 的**命名：

```
src/                        # ← 以下为代码 B 的布局（timeless/crates/timeless-app/src/）
├── combat.rs      # 战斗：Health/Damage、攻击属性、意图/状态组件、火球/爆炸 + 结算系统
├── movement.rs    # 移动：Position、位移行动、投射物飞行（位置+速度）；不含火球/爆炸
├── menu.rs        # 菜单：能力标记 + 技能表 + 键盘/面板消息 + 单一职责输入系统
├── timeline.rs    # 时间线：动作实体调度（虚拟时间驱动，无回合/阶段状态机）
├── display/       # 展示层子域：camera / unit / hints / hud / map
├── setup.rs       # 场景组合（地面/装饰/单位/HUD）
├── debug.rs       # 调试面板
└── main.rs        # App 装配与系统链
```

战斗数据建模遵循「实体 + 小组件组合」：`Health` 对应输出侧的 `PhysicalDamage`、
攻击属性拆成 `AttackFrame` / `AttackRange` / `Impact`、行动用**动作实体**表达
（载荷组件 + `ScheduledAction` + `Declared`/`Pending`/`Committed`，经
`Time<Virtual>` 时间线持续调度 + 两阶段结算），动作实体用 `bsn!` 构建。
A 与 B 在这一点上一致，A 的具体组件清单见
[ecs-combat-components.md](ecs-combat-components.md)。

（**已作废**：旧版这里写「由能力标记 + 技能表声明」——代码 A 用 `SKILLS` 注册表 +
`MenuSelection`，没有 `Can*` 能力标记；B 有 `Can*`。`bsn!` 构建动作实体在 A 中成立。）

## 现状与差距（WIP）

- **代码 A 是主线**，已迁入 B 的能力（翻滚 / 招架 / 火球 / 精力 / 技能菜单 /
  两阶段结算 + 三层裁决 / 无回合时间线），`cargo test` 97 通过。
  它的 backlog 见 [../status.md](../status.md)。
- **代码 B（`timeless/`）已冻结**：本检出跑不起来
  （`timeless/crates/timeless-app/assets/` 目录不存在、`vendor/parley` 补丁不存在）。
  B 仍然独有的只有 `bevy_egui` 调试面板与中文 HUD。
- 待落地：动作数值从硬编码常量外置成 `.ron`；`ActionTemplate` 资产图
  （`ActionId` → `ActionTemplate` 索引）；`PendingHit` 实体化。
