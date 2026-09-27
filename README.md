# Project Timeless

基于 **Bevy 0.19** 的 roguelike 策略游戏原型：**无回合**战斗时间线——谁能决策由各自的
`DecisionSlot` 决定，每个动作自带前摇 + 后摇，世界在暂停原因集合非空时冻结
（`Time<Virtual>`）。

```bash
cargo run    # 体素地形 + 世界空间战斗
cargo test   # 全绿是提交前提（见下）
```

## 从这里读

| 想知道 | 去哪 |
| :--- | :--- |
| **现在做什么**（活待办，一屏看完） | [`TODO.md`](TODO.md) |
| **某个领域还有哪些条目**（正文：现象 / 根因 / 改法 / 验收） | [`docs/backlog/`](docs/backlog/) |
| **以前做了什么、凭什么说做完了**（历史证据） | [`CHANGELOG.md`](CHANGELOG.md) |
| 文档入口与维护规则 | [`docs/index.md`](docs/index.md) |
| 命令 / 编码风格 / 提交规范（改动前必读） | [`AGENTS.md`](AGENTS.md) |
| 架构、域地图与十二条铁律 | [`docs/domain.md`](docs/domain.md) |
| 写任何 Bevy 代码之前 | [`docs/bevy-019.md`](docs/bevy-019.md) |

提交前三条必须全过：`cargo test` 全绿 · `cargo clippy --all-targets -- -D warnings` 零警告 ·
`cargo fmt --check` 通过。
