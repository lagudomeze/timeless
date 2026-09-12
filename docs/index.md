# Project Timeless 文档索引

> 本文件是项目文档的唯一入口。项目处于 WIP 阶段，文档与代码都可大胆调整；改动后请同步本索引。

## 项目定位

**无回合**（所有单位能决策就决策，仅玩家等待输入时冻结虚拟时间）+ 戴森球式供应链 +
信息即力量的 roguelike 策略游戏，代号 Project Timeless，基于 Bevy 0.19 实现。
战斗结算用**真实距离**，决策与同格判定按**格子**（见
[design/timeline-turnless.md](design/timeline-turnless.md)）。

## 文档地图

| 主题 | 文档 | 描述对象 |
| :--- | :--- | :--- |
| **进度与决策记录（唯一进度真相）** | [status.md](status.md) | 全部 |
| 游戏设计总纲 | [design/game-design.md](design/game-design.md) | 设计稿 |
| **无回合时间线详细设计（当前权威）** | [design/timeline-turnless.md](design/timeline-turnless.md) | 代码 A |
| 根目录 app 原型的领域化模块设计 | [design/app-modules.md](design/app-modules.md) | 代码 A |
| ECS 战斗组件化设计 | [design/ecs-combat-components.md](design/ecs-combat-components.md) | 代码 A |
| 架构原则与分层 | [design/architecture.md](design/architecture.md) | 设计稿 |
| 时间线系统详细设计（历史，已被取代） | [design/timeline.md](design/timeline.md) | 代码 B（冻结） |
| 时间线核心 v0.1 接口签名（历史） | [design/timeline-core-design.md](design/timeline-core-design.md) | 设计稿 |
| 旧 app → 根目录迁移记录（历史） | [design/app-migration.md](design/app-migration.md) | 代码 A 的前身 |
| 旧内容盘点与重设计（历史） | [design/app-redesign.md](design/app-redesign.md) | 设计稿 |
| Action Graph 设计（未落地） | [bevy/action-graph.md](bevy/action-graph.md) | 未来设计稿 |
| Bevy 0.19 速查与最佳实践 | [bevy/bevy-019.md](bevy/bevy-019.md) | 通用 |
| 免费素材获取与接入 | [art/assets.md](art/assets.md) | 通用 |
| Agent 协作规范 | [agent-guidelines.md](agent-guidelines.md) | 流程 |

> 「描述对象」是 [`status.md`](status.md) 第七节第 1 条强制的口径：
> 每篇文档必须说清自己描述的是**代码 A**（`src/`，主线）、**代码 B**（`timeless/`，冻结）
> 还是**未来设计稿**。`design/app-modules.md` 的旧版曾描述已删除的 We-Go 版本，
> 现已按代码 A 重写。

仓库入口指南见 [../AGENTS.md](../AGENTS.md)（不随本套文档自动同步，如需更新请手动维护）。

## 可复用 Skill

项目技能源码存放在 [../skills](../skills)，并安装到 `$CODEX_HOME/skills` 供本地 Codex 使用：

| Skill | 用途 | 使用时机 |
| :--- | :--- | :--- |
| `bevy-019-docs` | 查证 Bevy 0.19 API / 最佳实践，来源优先级与已知差异 | 任何 Bevy 代码编写、审查、迁移 |
| `bevy-assets` | 免费 2D/3D/音频素材获取、许可证检查、接入 Bevy | 素材选型、下载、入库、接入 |

安装与维护方式见 [agent-guidelines.md](agent-guidelines.md)。

## 版本基线

- Bevy `0.19.1`；文档与代码示例一律以 0.19 API 为准，引用前须核对官方来源。
- 对话记录或旧文档中的示例若与 0.19 冲突（如 `EventReader`、`SceneBundle`），以本文档与官方资料为准。
- 依赖版本索引与镜像环境注意事项见根目录 `../TODO.md`。
