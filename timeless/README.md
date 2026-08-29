# Project Timeless（暂定）

Roguelike 策略游戏：结合《黑神话》的「见招拆招」与《ToME4》的硬核策略。
战斗采用**同步时间线（We-Go）**：所有单位同时规划行动，时间连续推进；
玩家拥有**时间线特权**——攻击前摇窗口内检测到威胁时，可消耗精力执行
**翻滚取消（Roll Cancel）**中断攻击并闪避。

## 架构

```
timeless/                     # cargo workspace
├── crates/timeless-domain/   # 领域层：纯 Rust，零 Bevy 依赖（战斗裁决 / 网格）
└── crates/timeless-app/      # 应用层：Bevy 组件、系统、渲染
└── docs/timeline-core-design.md  # 时间线核心设计 v0.1
└── TODO.md                   # 项目管理清单 + 依赖/文档索引
```

分层约束：领域层可被 `cargo test` 独立覆盖；应用层只做编排，不含伤害公式。

## 快速开始

```bash
cd timeless
cargo test --workspace   # 领域层 11 个测试
cargo run -p timeless-app
```

> 本机 crates.io 直连不可用，依赖经清华镜像解析（见 TODO.md「环境注意事项」）。

## 引擎与依赖

- Bevy 0.17（docs: https://docs.rs/bevy/0.17.3 · 官方: https://bevy.org/learn/）
- 完整版本索引见 TODO.md

## 进度

Phase 0（workspace/分层/测试）✅ → Phase 1（可玩纵向切片）进行中。
