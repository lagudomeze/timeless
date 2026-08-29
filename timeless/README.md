# Project Timeless（暂定）

Roguelike 策略游戏：结合《黑神话》的「见招拆招」与《ToME4》的硬核策略。
战斗采用**同步时间线（We-Go）**：所有单位同时规划行动，时间连续推进；
玩家拥有**时间线特权**——攻击前摇窗口内检测到威胁时，可消耗精力执行
**翻滚取消（Roll Cancel）**中断攻击并闪避。

## 架构

```
仓库根目录：
├── TODO.md                   # 项目管理清单 + 依赖/文档索引
├── docs/design/              # 设计文档（含 timeline-core-design.md v0.1）
timeless/                     # cargo workspace
├── crates/timeless-domain/   # 领域层：纯 Rust，零 Bevy 依赖（战斗裁决 / 网格）
└── crates/timeless-app/      # 应用层：Bevy 组件、系统、渲染
└── vendor/parley/            # 本地补丁：CJK 分词（README.patch.md）
```

分层约束：领域层可被 `cargo test` 独立覆盖；应用层只做编排，不含伤害公式。

## 快速开始

```bash
cd timeless
cargo test --workspace   # 领域层 11 个测试
cargo run -p timeless-app
```

> 本机 crates.io 直连不可用，依赖经清华镜像解析（见根目录 `../TODO.md`「环境注意事项」）。

## 引擎与依赖

- Bevy 0.19.1（docs: https://docs.rs/bevy/0.19.1 · 官方: https://bevy.org/learn/）
- 完整版本索引见根目录 `../TODO.md`

## 进度

Phase 0（workspace/分层/测试）✅ → Phase 1（可玩纵向切片）✅ →
Phase 1.5（伪 3D 场景）✅ → Phase 1.6（模块化重构）✅ → Phase 2（技能与反馈）进行中。
