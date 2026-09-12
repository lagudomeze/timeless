# Project Timeless（暂定）

> **描述对象：代码 B（`timeless/` workspace）。**
> ⚠️ **B 已冻结**：能力已迁入仓库根目录的代码 A（`src/`），本检出中 B **跑不起来**——
> `crates/timeless-app/assets/` 目录不存在，`vendor/parley/` 补丁也不存在。
> 当前主线与进度见根目录 [../TODO.md](../TODO.md) 与 [../docs/status.md](../docs/status.md)。
> 下面描述的 We-Go 同步时间线与时间线特权是 **B 早期的设计意图**，
> 与 B 的实际代码（无回合）以及 A 的现状都不一致，仅作留档。

Roguelike 策略游戏：结合《黑神话》的「见招拆招」与《ToME4》的硬核策略。

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
cargo test --workspace   # 领域层 7 + 应用层 2 = 9 个测试（B 冻结前的口径）
cargo run -p timeless-app   # ⚠️ 会因缺资产报错，见顶部说明
```

> 本机 crates.io 直连不可用，依赖经清华镜像解析（见根目录 `../TODO.md`「环境注意事项」）。

## 引擎与依赖

- Bevy 0.19.1（docs: https://docs.rs/bevy/0.19.1 · 官方: https://bevy.org/learn/）
- 完整版本索引见根目录 `../TODO.md`

## 进度

Phase 0（workspace/分层/测试）✅ → Phase 1（可玩纵向切片）✅ →
Phase 1.5（伪 3D 场景）✅ → Phase 1.6（模块化重构）✅ → Phase 2（技能与反馈）进行中。
