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

- **描述对象**：本仓库有**两棵代码树**——主线是仓库根 `src/`（package `app`，下称代码 A），
  `timeless/` workspace 是已冻结的代码 B。任何文档 / 注释提到实现时都要说清是哪一棵
  （见 [status.md](status.md) 第七节）。
- 领域层零 Bevy 依赖：代码 A 是 `src/combat/formula/domain.rs`，代码 B 是
  `timeless-domain` crate；应用层不含伤害公式。
- 注释中文、标识符英文；`rustfmt` 默认配置；提交用 Conventional Commits（`feat:` / `fix:` / `docs:` 等）。
- 提交前必须通过（**代码 A，在仓库根目录**）：
  `cargo test` 全绿、`cargo clippy --all-targets -- -D warnings` 零警告、`cargo fmt --check` 通过。
  注意根 `Cargo.toml` **不是** workspace，`--workspace` 在这里没有意义；
  代码 B 的 `cargo test --workspace` 要在 `timeless/` 下跑（且 B 已冻结，不作为验收门槛）。
- 本机 crates.io 直连不可用：不用 `cargo add`，依赖手动写入 `Cargo.toml` 并在根目录 `../TODO.md` 版本索引表登记。
- 素材入库必须附许可证记录（见 [art/assets.md](art/assets.md)）。
- **不要用 `#[ignore]` 隐藏失败**：要么修好，要么在 `TODO.md` 写明根因与下一步。
