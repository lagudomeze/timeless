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
├── combat.rs      # 战斗：Health/Damage、攻击属性、意图/状态组件 + 结算系统
├── movement.rs    # 移动：Position、位移意图、投射物（火球）组件与系统
├── menu.rs        # 菜单：Action（UI 选项）+ 输入系统（写入意图组件）
├── timeline.rs    # 时间线：回合阶段机
├── display/       # 展示层子域：camera / unit / hints / hud / map
├── setup.rs       # 场景组合（地面/装饰/单位/HUD）
├── debug.rs       # 调试面板
└── main.rs        # App 装配与系统链
```

战斗数据建模遵循「实体 + 小组件组合」：`Health` 对应 `Damage`、攻击属性拆成
`AttackFrame` / `AttackRange` / `Impact`、行动用意图组件表达
（`Attack` / `Move` / `Roll` / `Fireball`，由能力标记 + 技能表驱动），详见
[ecs-combat-components.md](ecs-combat-components.md)。

## 现状与差距（WIP）

- `timeless/` 已具备 Phase 1.5 纵向切片：Message 体系、AI 意图、时间推进、
  翻滚取消、21×21 伪 3D 纸片场景、组件化战斗与调试面板（详见根目录 `../TODO.md`）。
- 待迁移：`ActionId(&'static str)` → `ActionTemplate` 资产索引；旧 `Event` 写法按 0.19 选型规则校正；`timeline-core-design.md` v0.1 与 Action Graph 的差异以新设计为准。
