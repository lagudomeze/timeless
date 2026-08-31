# 架构原则与分层

## 分层

| 层 | 载体 | 职责 | 约束 |
| :--- | :--- | :--- | :--- |
| 领域层 | `timeless-domain` | 战斗裁决、网格、时间线结算（纯 Rust） | **零 Bevy 依赖**，可独立单测 |
| 应用层 | `timeless-app` | Bevy 组件、系统、渲染、HUD 编排 | 不含伤害公式与结算逻辑 |
| 表现层 | `timeless-app` 内 | 输入、渲染、UI、调试面板 | 只消费领域结果，不反推规则 |

## 通信策略（Bevy 0.19）

- **Message**（`#[derive(Message)]` + `app.add_message::<T>()`，经 `MessageWriter` / `MessageReader`）：系统间**解耦、批量、有序**的缓冲队列；双缓冲存活 2 帧，不消费会被静默清理。
- **Event / EntityEvent + Observer**（`GlobalTrigger` / `EntityTrigger`，`On<T, B>`，`commands.trigger` / `trigger_targets`）：**即时响应、定向实体**。
- **UI 输入只翻译、不执行**：键盘 / egui 面板把操作翻译成 Message（`SelectSkill`、
  `MoveInput`、`CommitAction`、`ReactionInput`），由对应领域的单一职责系统消费落地；
  输入系统不得同时做决策与状态修改。
- **消息与其消费系统同属一个领域文件**：`MoveInput` + `move_input_system` 在
  `movement.rs`、`ActionsCommitted` + `finalize_declared_actions` 在 `timeline.rs`；
  其他领域需要该操作时只写消息，不重复实现。

选型规则：需要立即生效或针对具体实体 → Event + Observer；批量广播、允许晚一帧、强调解耦 → Message。`MessageWriter` 发送不会触发 Observer，两者不可混用。

> `EventReader` / `EventWriter` 在 0.19 已移除，旧代码一律迁移到上述两套体系。

## 数据驱动

- 组件只存数据；**可配置类型用 ID / usize 索引 / Handle** 引用，不用空标记组件充当身份。
- 技能、怪物、标签反应等一律走外部配置（serde + ron / Asset），应用层不硬编码数值。
- 动作定义资产化为 `ActionTemplate`（详见 [../bevy/action-graph.md](../bevy/action-graph.md)）。

## 代码组织

- `main.rs` 只负责装配 `App`；业务逻辑放 `lib.rs` 的 `GamePlugin`。
- Plugin 之间严格隔离，不互相 import；跨模块只经 Message / Event 通信。
- 测试用 `App::new()` + `MinimalPlugins` + `AppPlugin` 搭最小 App。

## 应用层文件布局（timeless-app）

按领域一文件组织，**不设独立的 systems 目录**——一个领域文件里组件、消息、
系统同放（高内聚）；只有 `display` 按表现子域拆成 mod 目录：

```
src/
├── combat.rs      # 战斗：Health/Damage、攻击属性、意图/状态组件、火球/爆炸 + 结算系统
├── movement.rs    # 移动：Position、位移行动、投射物飞行（位置+速度）；不含火球/爆炸
├── menu.rs        # 菜单：能力标记 + 技能表 + 键盘/面板消息 + 单一职责输入系统
├── timeline.rs    # 时间线：动作实体调度（虚拟时间驱动，无回合/阶段状态机）
├── display/       # 展示层子域：camera / unit / hints / hud / map
├── setup.rs       # 场景组合（地面/装饰/单位/HUD）
├── debug.rs       # 调试面板
└── main.rs        # App 装配与系统链
```

战斗数据建模遵循「实体 + 小组件组合」：`Health` 对应 `Damage`、攻击属性拆成
`AttackFrame` / `AttackRange` / `Impact`、行动用**动作实体**表达
（载荷组件 + `ScheduledAction` + `Declared/Pending/Committed`，由能力标记 +
技能表声明，经 `Time<Virtual>` 时间线持续调度 + 两阶段结算），动作实体用 `bsn!` 构建，
详见 [ecs-combat-components.md](ecs-combat-components.md)。

## 现状与差距（WIP）

- `timeless/` 已具备纵向切片：Message 体系、AI 意图、虚拟时间调度（无回合）、
  实时翻滚取消 / 招架、21×21 伪 3D 纸片场景、组件化战斗与调试面板
  （详见根目录 `../TODO.md`）。
- 待迁移：`ActionId(&'static str)` → `ActionTemplate` 资产索引；旧 `Event` 写法按 0.19 选型规则校正；`timeline-core-design.md` v0.1 与 Action Graph 的差异以新设计为准。
