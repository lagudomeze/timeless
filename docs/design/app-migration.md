# 旧项目 → `crates/app` 增量迁移路线图

> 目标：把旧 `timeless-app` + `timeless-domain` 的内容一点点迁移进新
> `crates/app`（Bevy 0.19 + BSN 场景体系），迁移过程中每个里程碑保持
> `fmt` / `clippy` / `test` 全绿。旧 crate 在完全迁移前**保留**作对照，
> 迁移完成后由用户决定何时删除。

## 迁移原则

1. **纯逻辑进 `app/src/domain/`**：零 Bevy 依赖、可独立单测；日后若想恢复
   crate 级隔离，整目录抽成 workspace crate 即可（`glam` 需转直接依赖）。
2. **引擎代码按「小组件 + 对应系统」拆分**：旧 `combat.rs` / `menu.rs` /
   `movement.rs` 里混在一起的属性、标记、消息、AI、结算、日志等拆成独立文件；
   组件多用 `#[derive(Component, Default, Clone)]`（供 `bsn!`），含 `Entity`
   字段的派生 `FromTemplate`，实体骨架用场景函数 + `bsn!` 表达。
3. **表现层不照搬**：纸片单位 / 悬停读数 / egui 面板等旧 hack 是否保留另行
   拍板；新场景、模型加载走 glTF `WorldAssetRoot`（见 `bevy/bsn` 参考）。

## 内容盘点与目标落点

### `timeless-domain`（纯逻辑）

| 旧文件 | 内容 | 新落点 | 状态 |
| :--- | :--- | :--- | :--- |
| `combat.rs` | `AttackStats` / `Side` / `HitOrder` / `HitResult` + `resolve_attack` / `resolve_combat`（含 7 个单测） | `app/src/domain/combat.rs` | ✅ T1 |

### `timeless-app`（引擎层）

| 旧文件 | 内容（现状混合） | 建议拆分落点 | 状态 |
| :--- | :--- | :--- | :--- |
| `timeline.rs` | `ScheduledAction` / `Declared` / `Pending` / `Committed`、`ResetBattle` / `ActionsCommitted`、reset / finalize / scheduler | `app/src/action/`（`scheduled.rs` + `states.rs` + `scheduler.rs`） | 待 T2 |
| `movement.rs` | `Position` / `GridMath`、`MoveTo` / `Roll`、`Projectile` / `LinearVelocity` / `Destination`、`MoveInput` + 输入/执行/飞行系统（2 个单测） | `app/src/movement/` | 待 T2/T3 |
| `combat.rs` | 属性组件（`Health` / `Stamina` / `Damage` / `AttackFrame` / `AttackRange` / `Impact`）、阵营标记（`Player` / `Enemy`）、防御标记（`Dodging` / `Parrying`）、载荷（`Attack` / `Parry` / `Fireball` / `ExplosionDamage`）、`CombatResult` / `BattleLog`、消息、AI + 两阶段结算 + 爆炸/死亡/日志系统 | `app/src/combat/`（按 stats / sides / attacks / defense / fireball / resolution / ai / death / log 拆小文件） | 待 T2/T3 |
| `menu.rs` | `Can*` 能力标记、`SKILLS` 技能表与消耗、`MenuSelection`、输入/提交/反应系统 | `app/src/menu/` | 待 T4 |
| `setup.rs` / `display/*` | 纸片单位、Billboard、HUD、悬停、箭头 | 多数被新场景体系取代；HUD/日志另议 | 待 T4/取舍 |
| `debug.rs` | egui 调试面板 | 是否引入 egui 依赖待定 | 取舍 |
| `main.rs` | 消息/系统装配 | 随各模块迁入 `app/src/main.rs` + `lib.rs` | 贯穿 |

## 里程碑建议（每个独立编译 + 提交）

- [x] **T1 纯裁决逻辑**：`domain/combat.rs` + 单测（本提交）。
- [ ] **T2 基础组件与调度**：坐标/网格数学、单位属性与阵营标记、动作实体
      （`ScheduledAction` + 状态标记）、虚拟时间调度（finalize / scheduler）；
      先给出纯逻辑单测 + 最小 App 冒烟。
- [ ] **T3 战斗闭环**：AI、攻击/防御载荷、两阶段结算、移动/翻滚/火球/投射物
      执行器、死亡检查与战斗日志——拆成小系统文件。
- [ ] **T4 输入与展示**：技能菜单、提交/反应输入（键盘 → 消息），HUD / 日志 /
      调试面板按新场景体系重建或取舍。
- [ ] **收尾**：旧 `timeless-app` / `timeless-domain` 内容迁移清零后移除，
      更新根 `AGENTS.md` / `TODO.md` / `docs/` 索引。
