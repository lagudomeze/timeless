# Project Timeless — 文档入口

> 本文件是项目文档的**唯一入口**。改动文档后同步本页的表格。

## 项目定位

**无回合**战斗时间线 + 戴森球式供应链 + 「信息即力量」的 roguelike 策略游戏，
基于 **Bevy 0.19.1**。

节奏来自两件事：每个单位自己的 `Ready`（能决策就决策）与每个动作自带的前摇 + 后摇；
世界只在**玩家等待输入**时冻结。结算用**真实距离**，决策与同格判定按**格子**。

## 当前状态

代码在仓库根目录 `src/`（package `app`）：

```bash
cargo test                              # 154 通过（152 单元 + 2 资产验收）/ 0 跳过
cargo clippy --all-targets -- -D warnings   # 必须零警告
cargo fmt --check
cargo run                                # 体素地形 + 世界空间战斗
```

进度与 backlog 只有一处：根目录 [`TODO.md`](../TODO.md)。

## 文档地图

| 主题 | 文档 | 什么时候读 |
| :--- | :--- | :--- |
| **架构与模块** | [architecture.md](architecture.md) | 想知道「这个功能该放在哪」「谁依赖谁」 |
| **无回合时间线** | [timeline.md](timeline.md) | 改战斗节奏、行动、防御、投射物、AI |
| **组件 → 系统对照** | [components.md](components.md) | 想知道「这个组件被谁读 / 写」或设计新零件 |
| 游戏设计总纲 | [game-design.md](game-design.md) | 想知道「为什么做这个机制」 |
| 素材与字体 | [assets.md](assets.md) | 选素材、换贴图、动 HUD 文案 |
| Bevy 0.19 速查 | [bevy-019.md](bevy-019.md) | 写任何 Bevy 代码之前 |
| 进度与 backlog | [../TODO.md](../TODO.md) | 想知道「现在做到哪了」「下一步做什么」 |
| 仓库指南 | [../AGENTS.md](../AGENTS.md) | 命令、编码风格、提交规范 |

## 可复用 Skill

技能源码在 [`../skills`](../skills)，并安装到 `$CODEX_HOME/skills` 供本地 Codex 使用：

| Skill | 用途 | 使用时机 |
| :--- | :--- | :--- |
| `bevy-019-docs` | 查证 Bevy 0.19 API / 最佳实践、来源优先级与已知差异 | 任何 Bevy 代码编写、审查、迁移 |
| `bevy-assets` | 免费 2D / 3D / 音频素材获取、许可检查、接入 Bevy | 素材选型、下载、入库、接入 |

## 维护规则（防漂移）

1. **进度只写 `TODO.md`**：其他文档只写「设计与约定」，不写「做到哪了」。
2. **勾选要有验收证据**：命令输出、测试名或控制台片段；没有证据就保持 `[ ]`。
3. **不引用不存在的标识符**：文档里出现的类型名必须能在 `src/` 里 grep 到，
   否则明确标「设计稿」。
4. **改代码后同步三处**：受影响的 `architecture.md` / `timeline.md` / `components.md`
   对应段落。
5. **不用 `#[ignore]` 隐藏失败**：要么修好，要么在 `TODO.md` 写明根因与下一步。

## 版本基线

- Bevy `0.19.1`；文档与代码示例一律以 0.19 API 为准，引用前须核对官方来源。
- 旧对话记录或旧文档里的示例若与 0.19 冲突（如 `EventReader` / `SceneBundle`），
  以 [bevy-019.md](bevy-019.md) 与官方资料为准。
- 依赖版本索引与镜像环境注意事项见 [`../TODO.md`](../TODO.md)。
