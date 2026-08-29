# Action Graph 设计

## 动机

用一张统一的图描述所有实体（玩家、NPC、怪物）的行为：**一个行为 = 一张有向图**，输入、时机、状态与资源检查都表达为边上的条件，而不是散落的 if/else。

## 结构

- **节点 = `ActionTemplate`（Asset）**：动作的静态定义（相位时长、消耗、效果、可取消规则）。节点标识用 **usize 索引或 `Handle`**，不用 String ID。
- **边 = `TransitionCondition`**：
  - `OnInput`：按键 / 输入动作。
  - `InWindow`：处于某相位窗口（如前摇）。
  - `OnEvent`：响应 Message / Event（如威胁检测、受击）。
  - `StateCheck`：实体状态（架势、无敌帧、移动锁）。
  - `ResourceCheck`：弹药 / 精力 / 冷却。
- **控制器**：玩家用输入选边；NPC / 怪物用 `Random` 或策略权重选边。选择结果统一提交为意图，走同一条裁决管线。

## 时间模型

- **逻辑刻度（帧）是结算器中的指针 / 索引**，由 `Resolving` 队列驱动，不是物理时钟。
- 决策阶段：帧是静态窗口坐标（用于判断「是否在前摇窗口内」）。
- 执行阶段：帧只剩排序权重（谁先命中）。
- **`PendingHit` 物化为 Entity**：延迟命中、弹道、闪避、反制都挂到具体实体上，可被后续裁决取消或改写。

## 与 v0.1 的关系

`timeline-core-design.md` v0.1 提供了可编译的接口骨架，但以下约定以本设计为准：

| v0.1 | Action Graph |
| :--- | :--- |
| `ActionId(&'static str)` | `ActionTemplate` 资产 + usize/Handle 索引 |
| `CancelRule` 数据表 | 边上的 `InWindow` + `ResourceCheck` 条件 |
| `PhaseTransitionEvent`（旧 Event） | 按 0.19 选型规则迁移到 Message 或 Observer |

## 落地步骤

1. `ActionTemplate` 资产化（serde + ron，字段含 phases / cost / cooldown / cancelable_by / impact）。
2. 纯领域层实现图遍历与裁决器（可单测，零 Bevy 依赖）。
3. 玩家与 NPC 控制器分别接入；`PendingHit` 实体化。
4. 配置层（克制系数、元素反应）接入后做数值验收。

相关：设计动机见 [../design/game-design.md](../design/game-design.md)，分层约束见 [../design/architecture.md](../design/architecture.md)。
