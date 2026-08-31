# 仓库指南（Repository Guidelines）

Project Timeless 是基于 Bevy 的 roguelike 策略游戏，核心玩法为 We-Go（同步回合）战斗时间线。Cargo workspace 位于 `timeless/`；仓库根目录放置本指南、`TODO.md` 与 `docs/` 文档。

## 项目结构与模块组织

```
根目录：
├── AGENTS.md                  # 本指南
├── TODO.md                    # 里程碑、依赖版本索引、环境注意事项
├── docs/                      # 设计文档（design / bevy / art）
└── timeless/
timeless/
├── crates/timeless-domain/   # 领域层：纯 Rust，零 Bevy 依赖（战斗裁决、网格）
├── crates/timeless-app/      # 应用层：Bevy 组件、系统、渲染、HUD
└── vendor/parley/            # 本地补丁：CJK 分词（见 README.patch.md）
```

领域类型（`AttackStats`）定义在 `timeless-domain`；网格坐标直接使用 Bevy 的 `IVec2`
（应用层 `Position` / `Roll` / `Destination` 包装 `IVec2`，网格数学以扩展 trait 落在应用层）。

## 构建、测试与开发命令

以下命令均在 `timeless/` 目录下执行：

- `cargo run -p timeless-app` — 启动游戏。
- `cargo test --workspace` — 运行全部单元测试（领域层现有 11 个）。
- `cargo clippy --workspace` — 静态检查，必须零警告。
- `cargo fmt` / `cargo fmt --check` — 格式化代码 / 校验格式。

环境注意事项：本机 crates.io 直连不可用，依赖经清华镜像解析。不要使用 `cargo add`（已知兼容性问题）；依赖须手动写入 `Cargo.toml`，并在代码中引入前更新根目录 `TODO.md` 的版本索引表。

## 编码风格与命名规范

- 遵循 `rustfmt` 默认配置（4 空格缩进，edition 2024）。
- Rust 标准命名：函数、变量、测试用 `snake_case`；类型与枚举变体用 `CamelCase`。
- 注释与文档注释（模块级 `//!`、条目级 `///`）使用中文；标识符与提交信息使用英文。
- 严格分层：`timeless-domain` 绝不引入 Bevy；`timeless-app` 不包含伤害公式；模块间仅通过 Bevy `Message` 类型通信。
- 高内聚低耦合：每个领域文件只装自己的组件 / 消息 / 系统，组件只表达自己的职责，
  不给无关系统夹带状态。例如移动领域（`movement.rs`）只含网格坐标、位移行动与投射物飞行；
  火球 / 爆炸等战斗内容归 `combat.rs`，通过 `ProjectileArrived` 消息衔接。
- 动作实体化：行动 = 独立实体（载荷组件 + `ScheduledAction` + `Declared` / `Pending` /
  `Committed` 状态标记），调度器不感知载荷，新增动作只需新增载荷与执行器，不改调度器；
  复杂交互走两阶段结算（阶段 1 计算 `CombatResult`，阶段 2 统一应用）；暂停用
  `Time<Virtual>`，不要手写阶段门控。
- 实体构建优先用 BSN（`bsn!` + `spawn_scene`）：组件派生 `Default + Clone`
  （含 `Entity` 字段的派生 `FromTemplate`），多个组件组合成实体用场景语法，
  不用长元组 `spawn((...))`；字段值若非字面量，一律包 `{expr}`。
  含 `Handle` 字段或复杂子实体树的生成（投射物、战斗单位）暂可保持 `spawn` 元组，
  待 BSN 资产模板验证后迁移。

## 输入与通信规范

- **UI 输入只翻译、不执行**：键盘 / egui 面板等输入源不得直接修改游戏状态，只把按键或
  点击翻译成 Bevy `Message`（如 `SelectSkill` / `MoveInput` / `CommitTurn` /
  `ReactionSelect`），再由对应领域的单一职责系统消费并落地。禁止在输入系统里同时做
  「翻译 + 决策 + 改状态」。
- **消息定义与消费它的系统同属一个领域文件**：如 `MoveInput` 与 `move_input_system`
  在 `movement.rs`、`TurnCommitted` 与 `phase_advance_system` 在 `timeline.rs`。
  其他领域需要发起该操作时只写消息，不重复实现；不要在生产者文件里定义消费方领域的消息。
- 跨模块 / 跨阶段交互一律走 `MessageWriter` / `MessageReader`；需要立即生效、针对具体
  实体时才用 Event + Observer，两者不可混用。
- 新增消息必须在 `main.rs` 用 `add_message::<T>()` 注册，并在消息上注明「谁写、谁消费」；
  输入类消息由对应领域系统消费：`select_skill_system` / `commit_system` /
  `reaction_execution_system`（menu）、`move_input_system`（movement）、
  `phase_advance_system`（timeline）。

## 测试规范

- 单元测试写在源码旁的 `#[cfg(test)] mod tests` 中，主要集中在 `timeless-domain`。
- 测试名用描述性的 snake_case，例如 `layer1_speed_frame_decides_who_hits_first`。
- 使用 `assert_eq!`，断言意图不直观时附带简短说明。
- 提交前必须通过：`cargo test --workspace` 全绿、`cargo clippy --workspace` 零警告、`cargo fmt --check` 通过。

## 提交与 Pull Request 规范

采用 Conventional Commits：`feat:`、`fix:`、`docs:`、`refactor:`、`test:` 前缀（如 `feat: add fireball skill`），每次提交只含一个逻辑变更。Pull Request 需说明改动内容与动机、关联根目录 `TODO.md` 中的里程碑条目，并记录手工验证结果（如控制台「谁先命中」输出）。
