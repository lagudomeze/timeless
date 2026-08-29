# 仓库指南（Repository Guidelines）

Project Timeless 是基于 Bevy 的 roguelike 策略游戏，核心玩法为 We-Go（同步回合）战斗时间线。Cargo workspace 位于 `timeless/`，仓库根目录仅包含本指南与项目目录。

## 项目结构与模块组织

```
timeless/
├── crates/timeless-domain/   # 领域层：纯 Rust，零 Bevy 依赖（战斗裁决、网格）
├── crates/timeless-app/      # 应用层：Bevy 组件、系统、渲染、HUD
├── docs/                     # 设计文档，如 timeline-core-design.md
└── TODO.md                   # 里程碑、依赖版本索引、环境注意事项
```

领域类型（`GridPos`、`AttackStats`）定义在 `timeless-domain`，在 `timeless-app` 中以 newtype 包装成 Bevy 组件。

## 构建、测试与开发命令

以下命令均在 `timeless/` 目录下执行：

- `cargo run -p timeless-app` — 启动游戏。
- `cargo test --workspace` — 运行全部单元测试（领域层现有 11 个）。
- `cargo clippy --workspace` — 静态检查，必须零警告。
- `cargo fmt` / `cargo fmt --check` — 格式化代码 / 校验格式。

环境注意事项：本机 crates.io 直连不可用，依赖经清华镜像解析。不要使用 `cargo add`（已知兼容性问题）；依赖须手动写入 `Cargo.toml`，并在代码中引入前更新 `TODO.md` 的版本索引表。

## 编码风格与命名规范

- 遵循 `rustfmt` 默认配置（4 空格缩进，edition 2024）。
- Rust 标准命名：函数、变量、测试用 `snake_case`；类型与枚举变体用 `CamelCase`。
- 注释与文档注释（模块级 `//!`、条目级 `///`）使用中文；标识符与提交信息使用英文。
- 严格分层：`timeless-domain` 绝不引入 Bevy；`timeless-app` 不包含伤害公式；模块间仅通过 Bevy `Message` 类型通信。

## 测试规范

- 单元测试写在源码旁的 `#[cfg(test)] mod tests` 中，主要集中在 `timeless-domain`。
- 测试名用描述性的 snake_case，例如 `layer1_speed_frame_decides_who_hits_first`。
- 使用 `assert_eq!`，断言意图不直观时附带简短说明。
- 提交前必须通过：`cargo test --workspace` 全绿、`cargo clippy --workspace` 零警告、`cargo fmt --check` 通过。

## 提交与 Pull Request 规范

项目尚未纳入版本控制，暂无提交历史可循。初始化 Git 后建议采用 Conventional Commits：`feat:`、`fix:`、`docs:`、`refactor:`、`test:` 前缀（如 `feat: add roll cancel window`），每次提交只含一个逻辑变更。Pull Request 需说明改动内容与动机、关联 `TODO.md` 中的里程碑条目，并记录手工验证结果（如控制台「谁先命中」输出）。
