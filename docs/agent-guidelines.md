# Agent 协作规范

本文档约定 Agent（含 Codex）在本仓库工作的行为规则。入口索引见 [index.md](index.md)。

## 文档维护

- 新设计 / 新机制先写入对应文档再实现；改动后更新 [index.md](index.md) 的文档地图。
- Bevy 相关内容只写进 [bevy/bevy-019.md](bevy/bevy-019.md)，且引用前必须查证。
- 仓库入口 [../AGENTS.md](../AGENTS.md) 保持精简，只放高频命令与约定，不随大文档膨胀。

## Skill 使用与维护

- 任何 Bevy 代码编写 / 审查 / 迁移：必须使用 **`bevy-019-docs`**。
- 素材选型 / 下载 / 入库 / 接入：必须使用 **`bevy-assets`**。
- 源码位于 `skills/`；修改后运行 `quick_validate.py` 校验，并同步安装到 `$CODEX_HOME/skills`（Windows 默认 `C:/Users/<user>/.codex/skills`）。

## 知识来源优先级

1. Bevy GitHub（`examples/`、migration guides）。
2. [docs.rs/bevy/latest](https://docs.rs/bevy/latest/bevy/?search=) 搜索符号签名。
3. 官方发布说明 / [bevy.org](https://bevy.org)。
4. [taintedcoders Bevy 0.19](https://taintedcoders.com/bevy/tldr)（最佳实践补充，非官方）。
5. 对话记录与旧文档（仅参考，必须校正到 0.19）。

**禁止**：用模型训练记忆充当权威；照搬 0.16 / 0.14 及更早的代码（`EventReader`、`SceneBundle` 等已移除）。

## 仓库约定（沿袭 AGENTS.md / TODO.md）

- 领域层 `timeless-domain` 零 Bevy 依赖；应用层不含伤害公式。
- 注释中文、标识符英文；`rustfmt` 默认配置；提交用 Conventional Commits（`feat:` / `fix:` / `docs:` 等）。
- 提交前必须通过：`cargo test --workspace`、`cargo clippy --workspace`（零警告）、`cargo fmt --check`。
- 本机 crates.io 直连不可用：不用 `cargo add`，依赖手动写入 `Cargo.toml` 并在 `timeless/TODO.md` 版本索引表登记。
- 素材入库必须附许可证记录（见 [art/assets.md](art/assets.md)）。
