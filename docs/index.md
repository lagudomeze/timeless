# Project Timeless 文档索引

> 本文件是项目文档的唯一入口。项目处于 WIP 阶段，文档与代码都可大胆调整；改动后请同步本索引。

## 项目定位

无回合（`Time<Virtual>` 驱动的行动时间线）+ 戴森球式供应链 + 信息即力量的
roguelike 策略游戏，代号 Project Timeless，基于 Bevy 0.19 实现。

## 文档地图

| 主题 | 文档 |
| :--- | :--- |
| 游戏设计总纲 | [design/game-design.md](design/game-design.md) |
| 时间线系统详细设计 | [design/timeline.md](design/timeline.md) |
| 架构原则与分层 | [design/architecture.md](design/architecture.md) |
| 根目录 app 原型的领域化模块设计 | [design/app-modules.md](design/app-modules.md) |
| ECS 战斗组件化设计 | [design/ecs-combat-components.md](design/ecs-combat-components.md) |
| Bevy 0.19 速查与最佳实践 | [bevy/bevy-019.md](bevy/bevy-019.md) |
| Action Graph 设计 | [bevy/action-graph.md](bevy/action-graph.md) |
| 免费素材获取与接入 | [art/assets.md](art/assets.md) |
| Agent 协作规范 | [agent-guidelines.md](agent-guidelines.md) |
| 时间线核心 v0.1（接口签名，历史） | [design/timeline-core-design.md](design/timeline-core-design.md) |

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
