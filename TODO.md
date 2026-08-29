# Project Timeless — 项目路线图与依赖索引

> 本文档位于仓库根目录，是全项目唯一路线图。勾选状态以会话内 todo 为准，
> 本文档是持久化版本；每次实现后同步勾选并更新「验收」结果。

## 常用命令（在 `timeless/` 下执行）

```bash
cargo run -p timeless-app    # 启动游戏
cargo test --workspace       # 全部单元测试（领域层 11+）
cargo clippy --workspace --all-targets -- -D warnings  # 零警告
cargo fmt --check            # 格式校验
```

## 目录结构

```
根目录
├── AGENTS.md                     # 仓库指南（构建/风格/提交规范）
├── TODO.md                       # 本文件：路线图 + 依赖索引
├── docs/                         # 设计文档
│   ├── design/                   #   游戏设计 / 时间线 / ECS 组件化 / 架构
│   ├── bevy/                     #   Bevy 0.19 速查 / Action Graph
│   ├── art/                      #   素材获取与接入
│   └── index.md                  #   文档索引（唯一入口）
├── skills/                       # 可复用 Codex 技能（bevy-019-docs / bevy-assets）
└── timeless/                     # cargo workspace
    ├── crates/timeless-domain/   # 领域层：纯 Rust，零 Bevy 依赖
    ├── crates/timeless-app/      # 应用层：Bevy 组件/系统/渲染
    ├── vendor/parley/            # 本地补丁：CJK 分词（README.patch.md）
    └── README.md
```

## 开发规范（沿袭 AGENTS.md / docs/design/architecture.md）

1. **分层解耦**：`timeless-domain` 绝不引入 Bevy；应用层不含伤害公式。
2. **数据驱动**：数值/技能/标签反应走外部配置（Phase 2.1，serde+ron），应用层不硬编码。
3. **消息通信**：模块间用 Bevy `Message`；组件/消息/系统同属一个领域文件（高内聚）。
4. **文档先行**：任何 Bevy API 使用前先查 docs.rs / 官方示例（见 `skills/bevy-019-docs`），
   不依赖训练记忆；本项目固定 Bevy 0.19.1。

## 依赖与文档索引

> 检索方式：本机 crates.io 直连被网络阻断，版本经**清华镜像稀疏索引**查询确认。
> ⚠️ 依赖一律手动写入 `Cargo.toml`（`cargo add` 在镜像下不可用）；新依赖确认版本后先登记下表再引入。

| 依赖 | 版本（最新稳定） | crates.io | docs.rs | 用途 / 引入阶段 |
| :--- | :--- | :--- | :--- | :--- |
| bevy | 0.19.1 | https://crates.io/crates/bevy | https://docs.rs/bevy/0.19.1 | 引擎（已引入，Phase 1） |
| bevy_egui | 0.40.1 | https://crates.io/crates/bevy_egui | https://docs.rs/bevy_egui/0.40.1 | 调试面板运行时（已引入） |
| bevy-inspector-egui | 0.37.0 | https://crates.io/crates/bevy-inspector-egui | https://docs.rs/bevy-inspector-egui/0.37.0 | egui 依赖（已引入） |
| icu_segmenter | 2.3.0（features=auto） | https://crates.io/crates/icu_segmenter | https://docs.rs/icu_segmenter/2.3.0 | CJK 分词（已引入，配 vendor/parley 补丁） |
| serde | 1.0.x | https://crates.io/crates/serde | https://docs.rs/serde/latest | 配置序列化（Phase 2.1） |
| ron | 0.12.x | https://crates.io/crates/ron | https://docs.rs/ron/latest | .ron 配置格式（Phase 2.1） |

引擎官方文档：https://bevy.org/learn/ · 迁移指南：https://bevy.org/learn/migration-guides/

## 里程碑

### 已完成 ✅

- **Phase 0 项目搭建**：workspace 拆分（domain/app）、领域层 11 个单测、依赖版本检索。
- **Phase 1 纵向切片**：Message 体系、AI 意图、回合推进、翻滚取消、调试面板、控制台「谁先命中」。
- **Phase 1.5 伪 3D 场景**：21×21 草地地图 + Kenney 装饰、纸片单位（Billboard+贴地阴影）、
  中文 HUD（NotoSansSC + vendor/parley CJK 修复）、右键镜头、Tab 技能切换、WASD 斜向移动、
  Gizmos 移动箭头、悬停格坐标读数。
- **Phase 1.6 模块化重构**：display 拆 mod 目录（camera/unit/hints/hud/hover/map）、
  移动与战斗分域（movement.rs）、AttackStats → AttackFrame/AttackRange/Impact/Damage 小组件、
  意图组件（Attack/Move/Retreat/Dodge/Parry/Interrupted）、火球投射物（Projectile+Destination+ExplosionDamage）、
  死亡检查独立系统、ECS 组件化设计文档。

### Phase 2.0 — 技能与反馈（进行中）

- [x] **火球技能**：Tab 循环可选（攻击/移动/翻滚/火球），消耗 2 精力；提交后生成投射物
  （速度 4 格/秒、爆炸 8 伤害/半径 1），目标格在提交时锁定——敌人若本回合移动可躲避。
- [x] **招架反应**：Reaction 阶段新增「招架」（消耗 1 精力）：本回合免疫敌方攻击且不位移，
  与翻滚（位移+闪避）形成取舍。
- [x] **战斗日志 UI**：屏幕右下滚动显示最近战斗消息（提交/裁决/命中/闪避/招架/爆炸/胜负），
  替代纯控制台输出。
- [x] **精力回复**：每回合结算后回复 1 点（上限 5），支撑技能循环。
- [x] 验收：上述功能实机验证 + `fmt`/`clippy`/`test` 全绿。

### Phase 2.1 — 数据驱动动作（设计 v0.1 → v0.2 桥接）

- [ ] serde + ron 加载 `ActionTemplate` 配置（assets/actions/*.ron）：phases / cost / cooldown /
      hit_frame / impact / damage / range。
- [ ] `ActionId` → 配置索引（Handle 或 usize），应用层不再硬编码技能数值。
- [ ] 技能注册表资源（ActionRegistry），HUD/菜单从注册表读取选项与消耗。

### Phase 2.2 — 逻辑刻度时间线（docs/design/timeline.md v0.2）

- [ ] `ExecutionQueue` / `ScheduledAction`：按 hit_clock → distance → poise → actor 排序结算。
- [ ] 攻击动作三段式：前摇（可翻滚取消）/ 判定帧 / 后摇，`GlobalTime` 跳跃式推进。
- [ ] `PendingHit` 延迟命中物化（弹道飞行、延迟 AOE 排程），`CombatTimeline` 未来事件堆。
- [ ] 威胁提示改为「前摇窗口」表达（v0.1 的 DecisionPause 冻结降级为可选项）。

### Phase 3 — 策略深度与多单位

- [ ] 多敌人战斗：单例查询改为多实体查询，AI 每单位独立意图。
- [ ] 范围攻击（L2 排程）、冲刺（位移 2 格）、格挡减伤 + 架势槽。
- [ ] AI 威胁循环：进入射程→预读玩家意图→设防/闪避/格挡决策。
- [ ] 弹药分线（AmmoPouch）与技能冷却（Cooldowns），资源置换闭环。

### Phase 3.5 — 信息层（G 层：信息即力量）

- [ ] 洞察力：查看怪物数据（帧/射程/破势/血量）、帧窗口细节。
- [ ] 战斗日志回看（历史 N 回合）、死亡复盘（谁在哪个刻度命中了谁）。
- [ ] 成长以知识为主：升级解锁信息权限而非纯数值。

### 表现层待办

- [ ] 纸片角色贴图（带 alpha PNG 替换纯色占位；Kenney 角色包或 LPC 生成器）。
- [ ] 单位头顶状态回 3D（Text2d 在 Camera3d 下不渲染；需 2D 叠加相机或世界→屏幕投影，
      或等待 Bevy 世界空间 UI）。
- [ ] 开发热重载：`file_watcher` feature（dev profile）。
- [ ] 火球飞行轨迹/爆炸特效（Gizmos 或粒子）。

## 验收标准（每次提交前）

- [ ] `cargo test --workspace` 全绿
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 零警告
- [ ] `cargo fmt --check` 通过
- [ ] 实机冒烟：启动无 panic / 无资产错误 / 无 ICU4X 刷屏
