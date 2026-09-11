# Project Timeless — 进度统合与文档差异报告

> 状态：**当前唯一进度真相**（2026-09 会话）
> 本文取代散落在 `TODO.md` / `docs/design/*` / `docs/integration-status.md` 里的进度口径。
> 差异结论全部来自**逐行读码**；未验证的推断标注为「未验证」。
> 维护规则见文末「九、防漂移规则」。

---

## 一、最重要的结论：仓库里有两个并存的实现

这是所有「文档与代码冲突」的根因——文档没有声明自己描述的是哪一棵代码树。

| 代号 | 位置 | 包名 | 形态 | 证据 |
| :--- | :--- | :--- | :--- | :--- |
| **A. 根原型** | `src/`（仓库根） | `app` | 体素世界空间纵切，**We-Go 阶段机**（规划/推进） | `src/timeline/resources.rs:14-28`（`Phase::Planning` / `Resolving` + `round` + `window: Timer`） |
| **B. 旧 crate** | `timeless/crates/*` | `timeless-app` / `timeless-domain` | 网格化伪 3D，**无回合**（虚拟时间持续流动） | `timeless/crates/timeless-app/src/main.rs:3`「无回合设计：`Time<Virtual>` 持续流动」 |

补充两点归属判断：
- **`TODO.md` 的 Phase 1.5–1.13 全部描述 B**：那串条目里的 `Position` / `GridMath` / `Roll` / `Dodging` / `Fireball` / `Parrying` / 中文 HUD 都只存在于 B（见第三节）。
- **A 不属于那条 Phase 链**：A 是 `app-migration.md`（2026-09-10 更新）所指的「移到仓库根目录、按 `req0.MD` 领域化范式重构」的那棵树。
  仓库当前**没有 git 历史**（工作区不是仓库）可用来判定先后，因此**不要以「谁更新」为保留依据**，只看 D1 的工程取舍。

**关键事实：两棵树的时间线模型是相反方向的。**

| 维度 | A 根原型 `src/` | B 旧 crate `timeless/` |
| :--- | :--- | :--- |
| 阶段机 | `Phase::{Planning, Resolving}` + `round` + 1s 窗口 | 无回合、无阶段（虚拟时间默认流动） |
| 提交 | `ActionsCommitted` → `begin_resolution()` | `ActionsCommitted` → `finalize_declared_actions`（入队分配 `execute_at`） |
| 收尾 | `end_round_system` → `RoundEnded` 回到 Planning | 无 `RoundEnded` 语义（战斗结束才暂停） |
| 坐标 | 世界空间 `Transform` + `Vec3`，命中用真实距离 / 半径 | 网格 `Position(IVec2)` + `GridMath`，命中用切比雪夫距离 / 格 |
| 动作载荷 | `MoveAction` / `JumpAction` / `ShootAction` / `MeleeAction` | `MoveTo` / `Roll` / `Attack` / `Parry` / `Fireball` |
| 防御 | **无**（无翻滚 / 招架 / 闪避） | `Dodging`（i 帧过期）/ `Parrying`（绑定攻击实体） |
| 资源 | **无** `Stamina` | `Stamina`（翻滚 1 / 招架 1 / 火球 2，每轮回 1） |
| 战斗裁决 | 几何碰撞 + `PhysicalDamage` / `Armor` / `HitRadius` | 领域层 `resolve_combat` 三层裁决（帧→射程→破势）+ 两阶段结算 |
| 测试 | 49 个（`#\[test\]`，`src/**`） | 9 个（domain 7 + app 2） |

**因此：**
- `TODO.md` 的 Phase 1.13「删除 `TurnPhase` / 无回合化」**对 B 成立、对 A 不成立**。
- `docs/design/timeline.md`、`game-design.md` 顶部的「⚠️ 已过时：自 Phase 1.13 起无回合」警告**方向正确但归属不清**——读者会以为根原型也无回合。
- `docs/design/app-modules.md` 描述的**就是 A**，且与 A 的代码高度一致（逐条核对通过）。
- `docs/design/ecs-combat-components.md` 描述的是 **B 的组件名 + A 的目录名**，是两棵树**混合**后的产物，不能直接照着实现。

👉 **必须先拍板保留哪一棵（见第七节 D1）。** 在拍板前，任何文档都无法「正确」。

---

## 二、代码 A（根原型 `src/`，package `app`）的真实状态

### 已落地（读码确认）

| 领域 | 实际内容 |
| :--- | :--- |
| `world` | 体素数据域，**零渲染依赖**：`CHUNK_SIZE = 32`（`world/chunk/components.rs:8`）、`ChunkLoader { radius: IVec3 }`（默认 `(0,1,0)`，`components.rs:103-110`）、噪声地形纯函数 + `TerrainConfig`（`world/terrain/resources.rs`）、`ground_position` 贴地（`world/terrain/systems.rs:61`）、`MinimalPlugins` 可单测（`world/plugin.rs:40-44`） |
| `voxel_render` | 异步网格化（`AsyncComputeTaskPool` + 句柄挂实体）、面剔除（`MeshingConfig::cull_hidden_faces`）、按方块类型分组的 `ChunkSurface`、面朝向明暗（`voxel_render/lighting/systems.rs`） |
| `movement` | `Transform` + `Velocity` / `MoveSpeed`；行动载荷 `MoveAction{axis}` / `JumpAction` + `Jumping` 弹道；`ground_direction` 平面轴约定；窗口结束 `stop_on_round_end_system` |
| `combat` | `health` / `formula`（`PhysicalDamage` → `Armor` 减免 → `DamageEvent`）/ `attributes`（`Armor` / `HitRadius`）/ `targeting`（碰撞 + 近战扇形）/ `lifecycle`（`Projectile` / `HitOnce` / `Lifetime`）/ `skills`（`ShootAction` 箭矢、`MeleeAction` 横扫，声明→执行器）。**两阶段结算与 `CombatResult` 不存在于 A** |
| `timeline` | `ScheduledAction{actor, windup, execute_at}` + `Declared/Pending/Committed` + `Phase::{Planning,Resolving}` + `RESOLUTION_WINDOW = 1.0` + `pause_during_planning_system`（规划期冻结 `Time<Virtual>`）+ `clear_declared_actions` |
| `ai` | `EnemyBrain`（`engage_range` / `attack_range`）+ `AttackCooldown`（按轮递减）；只在规划阶段声明移动或射击，**无闪避 / 招架 / 撤退意图** |
| `input` | 只翻译：`player_move_input_system`（含 `GroundBasis` 屏幕→世界换算）、`player_skill_input_system`（Q 射 / E 近战 / Space 跳）、`player_commit_input_system`（Enter）、`pointer.rs`（中键 `PanCamera`） |
| `presentation` | 相机（`CameraRig` + `PanCamera`）、装饰（18 个 Kenney glTF 随机摆放）、`BattleLog` 战斗日志、HUD（英文，`ROUND` + `PLANNING/RESOLVING` + 双方状态 + 本轮声明） |
| `spawn` | 组装车间：`unit_scene` + `player.rs`（+`MoveSpeed`/`ChunkLoader`）+ `enemy.rs`（+`EnemyBrain`/`AttackCooldown`）+ `assembly.rs`（开局）+ `restart.rs`（`ResetBattle` 功能胶水） |

### A 已确认**不存在**（文档却仍在引用）

`Position` / `GridMath` / `IVec2` 网格坐标（`src/movement/mod.rs:8` 明说不造 `Position`）·
`Stamina` / `AmmoPouch` / `Cooldowns` / `Poise` · `Dodging` / `Parrying` · `Parry` / `Roll` / `Fireball`
载荷 · `CombatResult` / `HitLanded` / `PendingHit` / `CombatTimeline` / `ExecutionQueue` ·
`ActionId` / `ActionTemplate` / `CancelRule` / `CancelPrivilege` / `DecisionPause` ·
`menu.rs` / `Can*` 能力标记 / `SKILLS` 技能表 / `SelectSkill` / `ReactionInput` · `bevy_egui` 调试面板。

### A 的按键（与实现一致）

`WASD`/方向键 移动 · `Q` 射击 · `E` 近战 · `Space` 跳跃 · `Enter` 提交 · `R` 重置 · 中键拖拽平移相机。

### A 的具体缺陷（读码发现，非文档问题）

1. **单位是占位模型**：玩家 = `rock_largeA.glb`、敌人 = `tree_oak.glb`（`spawn/player.rs:23`、`spawn/enemy.rs:23`），注释自述「后续替换角色模型」。
2. **`assets/LICENSES.md` 登记了不存在的素材**：`fonts/NotoSansSC-Regular.otf` 在 `assets/` 下不存在（`assets/` 只有 21 个 glb + 1 个 png）；而 A 的 HUD 已经是纯英文（`presentation/hud.rs:5-6`），字体登记属于 B 的遗留。
3. **`textures/ground/grass.png` 未被任何代码引用**（grep `textures/` 在 `src/` 下 0 命中）。
4. **玩家没有体素碰撞 / 爬坡**：`TerrainConfig` 注释直言「单位目前不会跟着地形爬坡」（`world/terrain/resources.rs:26`），只在生成时贴地。
5. **AI 只会在「射程内」和「靠近」之间二选一**，没有威胁预读、没有设防，与设计文档的「怪物意图循环」差距最大。
6. **A 没有技能消耗 / 资源系统**，所以 `docs/design/timeline.md` 的「资源置换闭环」在 A 上无从谈起。

---

## 三、代码 B（`timeless/` workspace）的真实状态

| crate | 实际内容 | 测试 |
| :--- | :--- | :--- |
| `timeless-domain` | `combat.rs`：`AttackStats`（帧/射程/破势/伤害）+ `resolve_combat`（三层裁决）/ `resolve_attack` / `HitOrder` / `Side`。**仍是聚合结构**（`app-migration.md` 声称已拆掉，实际上只在 B 的应用层拆了小组件） | 7 |
| `timeless-app` | 单文件领域：`combat.rs`（805 行：`Stamina` / `Dodging` / `Parrying` / `Attack` / `Parry` / `Fireball` / `CombatResult` + 两阶段结算 + 爆炸 + `ai_system` + 日志）、`movement.rs`（`Position` / `GridMath` / `MoveTo` / `Roll` / `Projectile` / `LinearVelocity` / `Destination`）、`timeline.rs`（152 行：`ScheduledAction{execute_at, cast_duration, actor}` + `finalize_declared_actions`，**无阶段机**）、`menu.rs`（525 行：`Can*` + `SKILLS` + `SelectSkill` / `CycleSkill` / `CommitAction` / `ReactionInput`）、`setup.rs`、`debug.rs`（egui 面板）、`display/`（camera/hints/hover/hud/map/unit） | 2 |
| 依赖 | `bevy 0.19`、`icu_segmenter 2.3.0(auto)`、`bevy-inspector-egui 0.37`、`bevy_egui 0.40`；`icu_segmenter` / `bevy-inspector-egui` 已声明但**代码里 0 引用** |

### ⛔ B 在本检出中很可能跑不起来（读码验证）

1. **B 的素材目录整个不存在**：`timeless/crates/timeless-app/assets/` 下**没有任何文件**，
   而 `main.rs:25-28` 把 `AssetPlugin.file_path` 指向该目录。`setup.rs` / `display/*` 要加载
   草地贴图、18 个 Kenney glTF、纸片单位与 `fonts/NotoSansSC-Regular.otf`——全部缺失。
   → `cargo run -p timeless-app` 会大量报资产 error（这也正是「实机冒烟」那条迟迟无法打勾的原因）。
2. **`timeless/vendor/parley/` 不存在**，全仓库也没有 `README.patch.md`；
   `timeless/Cargo.lock` 里 `parley 0.9.0` 来自 registry，**没有 `[patch]` 段**。
   `AGENTS.md` / `timeless/README.md` / `TODO.md` 说的「本地补丁：CJK 分词」在本检出中不成立，
   而 `icu_segmenter 2.3.0(auto)` 作为「配合补丁」的依赖也就失去了理由（且未被任何代码引用）。
3. **B 没有 `lib.rs` / `GamePlugin`**：业务在 `main.rs` + 单文件领域里；`architecture.md:34`
   的「业务逻辑放 `lib.rs` 的 `GamePlugin`」对 B 不成立（只对 A 成立）。
4. **B 没有 `ai` 模块**：AI 是 `combat.rs:254` 的 `ai_system`；`AttackCooldown` 只存在于 A。

### B 的能力清单（比 A 强的地方，均读码确认）

> **B 的文档口径是领先的**：`docs/design/timeline.md`（无回合）与 `ecs-combat-components.md`
> （动作实体 / 防御标记 / 两阶段结算）描述的就是 B 的形态。**但 B 的进度不等于文档承诺**：
> B 里同样没有 `PendingHit` / `CombatTimeline` / `ExecutionQueue` / `ActionTemplate` /
> `CancelRule` / `CancelPrivilege` / `try_cancel` / `AmmoPouch` / `Cooldowns` / `Poise`，
> 也没有三段式前摇窗口；`GlobalTime` 同样不存在。B 的「无回合」是**实数 `Time<Virtual>` 调度**，
> 不是文档里的逻辑刻度。

`Position(IVec2)` + `GridMath::{chebyshev, retreat_from}` ·
`MoveTo{velocity}` / `Roll{from}` / `Attack` / `Parry{target_attack}` / `Fireball` / `ExplosionDamage` ·
`Health` / `Damage` / `AttackFrame` / `AttackRange` / `Impact` / `Stamina` ·
`Dodging{expires_at}`（`DODGE_MS = 500`）/ `Parrying` ·
`CombatResult` + `combat_phase1_system`（只读裁决）/ `combat_phase2_system`（统一应用）·
`parry_executor` / `fireball_executor` / `explosion_system` / `death_check_system`（战斗结束才 `time.pause()`）·
`menu.rs`：`Can*` 能力标记 + `SKILLS[4]` + `SelectSkill` / `CycleSkill`（Tab / Shift+Tab）/
`CommitAction` / `ReactionInput{RollCancel|Parry}` + `commit_system` 扣费 ·
`debug.rs` egui 面板（技能 / 提交 / 反应 / 重置按钮）· `display/` 21×21 网格 + 纸片 Billboard + 悬停读数 ·
消息 `HitLanded` / `ProjectileArrived`（A 无这两条）。

**测试数口径已过时**：`AGENTS.md` / `TODO.md` / `timeless/README.md` 都写「领域层 11 个测试」，
实际 domain **7** + app **2** = **9**。

**工作区结构陷阱**：根 `Cargo.toml` 是独立 package，**没有 `[workspace]`**；`timeless/Cargo.toml` 才是 workspace（`resolver = "2"`）。因此「在根目录跑 `cargo test --workspace`」只会跑 `app`，跑不到 `timeless/` 的 9 个测试——文档里的命令示例是有歧义的。

---

## 四、文档差异矩阵（逐文档）

| 文档 | 描述对象 | 与代码的关系 | 结论 |
| :--- | :--- | :--- | :--- |
| `docs/design/app-modules.md` | A | **一致**（领域表、目录树、We-Go 循环、按键、插件流水线逐条核对通过） | ✅ 保留，升格为 A 的权威结构文档 |
| `docs/design/architecture.md` | A + B 混 | 分层原则正确；但「应用层文件布局」列的是 B 的 `src/`（`combat.rs` / `menu.rs` 单文件），`现状与差距` 又写「虚拟时间调度（无回合）」——那是 B | ⚠️ 需按 A / B 分开改写 |
| `docs/design/ecs-combat-components.md` | B 的组件 + A 的 `timeline/` 目录名 | 组件清单（`Position` / `MoveTo: IVec2` / `Roll` / `Fireball` / `CombatResult`）在 A 中**全部不存在**；`调度与两阶段结算` 写的是 B 的 `combat_phase1/phase2` | ❌ 与 A 直接冲突；需标注「描述 B」 |
| `docs/design/timeline.md` (v0.2) | B 的形态 + 逻辑刻度 | 顶部已自我标注「已过时」；但正文仍是 We-Go 阶段机 + `CombatTimeline.clock` 逻辑刻度，B 与 A 都没实现 | ⚠️ 只能当「未来设计稿」，不能当现状 |
| `docs/design/timeline-core-design.md` (v0.1) | 未落地的接口签名 | `ActionId(&'static str)` / `CancelRule` / `try_cancel` / `CancelPrivilege` / `CombatPhase::DecisionPause` / `AmmoPouch` / `Cooldowns` / `Poise` —— A、B 均无 | ⚠️ 历史文档，`index.md` 已标「历史」 |
| `docs/bevy/action-graph.md` | 未落地的设计 | `ActionTemplate` / `TransitionCondition` / `PendingHit` 实体化均不存在 | ⚠️ 未来设计稿 |
| `docs/design/game-design.md` | B 的未来形态 | 顶部警告方向正确（针对 B）；正文「帧不是 Bevy 真实时间 / 逻辑刻度」与 A 的 `Time<Virtual>` 实数调度冲突 | ⚠️ 需注明适用树 |
| `docs/design/app-migration.md` | 迁移记录 | 已自标「路径已过期」；但「纯逻辑进 `app/src/domain/`」「已拆掉 `AttackStats`」与实际不符（A 无 `domain/`，B 仍有 `AttackStats`） | ⚠️ 历史记录，勿当现状 |
| `docs/integration-status.md` | 声称审计 A | **本身已过时**：说 `timeline.rs` 里「无 `ReactionInput` 系统」是对的 A，但把 B 的 `menu.rs` / `debug.rs` / `Position` / 防御标记写进了「代码结构总结」，还声称 `Phase` 有 `Resolving` 而「无 1s 窗口」——实际 `RESOLUTION_WINDOW = 1.0` 确实存在 | ❌ 已被本文取代 |
| `TODO.md` | A + B 混 | Phase 1.13「无回合化」对 A **不成立**；「Phase 2.0 火球 / 招架 / 战斗日志 / 精力回复 [x]」在 A 中**全部不存在**（属于 B）；「领域层 11 个单测」过时（实际 9） | ❌ 需重写 |
| `AGENTS.md` | 仓库指南 | 根 `src/` 描述为「世界空间纵切」正确；但「领域层现有 11 个」过时；未提及 `docs/status.md` / `docs/index.md` 未收录本文 | ⚠️ 小修 |
| `docs/index.md` | 索引 | **未收录** `app-migration.md`、`app-redesign.md`、`integration-status.md`；也缺 `game-design.md` 之外的进度入口 | ⚠️ 待补 |

### 冲突清单（按严重程度）

| # | 冲突 | 位置 | 影响 |
| :-- | :--- | :--- | :--- |
| C1 | **有回合 vs 无回合**：A 有 `Phase::Planning/Resolving` + 轮次 + 1s 窗口；`timeline.md` / `game-design.md` / `architecture.md` 都说「自 Phase 1.13 无回合」 | A 代码 vs 3 份设计文档 | 任何人按文档改 A 都会删掉正在工作的阶段机 |
| C2 | **网格 vs 世界空间**：A 用 `Transform` 真实距离；`ecs-combat-components.md` / `timeline.md` 用 `IVec2` 切比雪夫 | A 代码 vs 2 份设计文档 | 攻击范围 / 命中判定两套语义并存 |
| C3 | **验收勾选造假**：TODO/本文档旧版把「实机冒烟：无 panic / 无资产错误」标为 `[x]`，但同一份 TODO 又写「当前 TODO.md 列表标记为 [x] 待验证」 | `TODO.md:144` vs `TODO.md:191-196` | 验收信号不可信 |
| C4 | **Phase 2.0 成果归属错位**：火球 / 招架 / 战斗日志 / 精力回复是 B 的能力，却记在描述 A 的路线图里 | `TODO.md:135-144` | 读者以为 A 有资源系统与防御 |
| C5 | **`AttackStats` 是否已拆**：`app-migration.md` 说「不再出现 `AttackStats` 这类聚合结构」，B 的 domain 里仍在用 | `app-migration.md:29-32` vs `timeless-domain/src/combat.rs` | 领域层重构状态被误报 |
| C6 | **测试数**：11 vs 实际 9 | `AGENTS.md`、`TODO.md`、`timeless/README.md` | 小，但会污染「全绿」判断 |
| C7 | **素材登记**：`assets/LICENSES.md` 登记不存在的字体；根原型已无 CJK 需求 | `assets/LICENSES.md:9` | 素材可追溯性失真 |
| C8 | **文档索引漏项**：4 份文档不在 `index.md` 里 | `docs/index.md` | 老文档被当成现状读 |
| C9 | **按键写错（含运行期日志）**：`app-modules.md:44` 与 `src/timeline/mod.rs:22` 都写「`Space` 声明射击」，实际 `Q` = 射击、`Space` = 跳跃（`src/input/keyboard.rs:90,96`）；`src/timeline/systems.rs:21` 的 `info!` 也照抄了这句 | 2 处文档 + 1 处代码注释 + 1 处控制台日志 | 玩家按 Space 期望射击，实际起跳 |
| C10 | **「AI 直接写 `Velocity`」是错的**：`src/lib.rs:13`、`src/ai/mod.rs:3-4`、`app-modules.md:91` 都这么说，实际 `enemy_declare_system` 只 spawn 行动实体（`src/ai/systems.rs:48,53`），`Velocity` 由执行器写入（`movement/actions.rs:144`） | 3 处（含模块文档） | 与「AI 只声明、不落地」的核心约定自相矛盾 |
| C11 | **HUD「纯英文」规则被日志正文破坏**：`hud.rs:5-6` 与 `AGENTS.md` 要求英文，但 `HudLog` 渲染的 `BattleLog` 文本是中文（`presentation/log.rs:52-55,65`） | 代码内部不一致 | 默认字体无 CJK → HUD 里中文会显示为缺字方块 |
| C12 | **悬空引用**：`src/movement/events.rs:8` 指向不存在的 `apply_move_command_system`（真实消费者是 `declare_move_system`）；`src/combat/health/events.rs:19` 指向不存在的 `crate::scene::BattleLog`（真实为 `crate::presentation::BattleLog`） | 2 处代码注释 | 新人按注释找不到代码 |
| C13 | **`AGENTS.md` 命名了不存在的消息 / 系统**：`TurnCommitted` / `CommitTurn` / `phase_advance_system` 全仓库无定义；`SelectSkill` / `commit_system` 只在 B 里；且「新增消息必须在 `main.rs` 注册」与代码（各领域插件 `build` 注册）矛盾。`AGENTS.md` 的移动领域描述（网格坐标 + 投射物飞行 + `ProjectileArrived`）也是 B 的 | `AGENTS.md:59-77` | 仓库指南把 A/B 混成一条规则 |
| C14 | **`app-modules.md` 目录树漏文件**：`input/` 漏 `pointer.rs`、`presentation/` 漏 `hud.rs`、movement 行漏 `JumpCommand` | `app-modules.md:17,157,158` | 轻微，补上即可 |
| C15 | **`vendor/parley` 补丁不存在**：`AGENTS.md:17`、`TODO.md:31`、`timeless/README.md:17` 都声称有本地补丁与 `README.patch.md`；实际 `timeless/vendor/**` 无任何文件，`Cargo.lock` 里 `parley 0.9.0` 来自 registry，无 `[patch]` 段 | 3 处文档 vs 文件系统 | 「CJK 修复」这条历史事实在本检出中不成立 |
| C16 | **B 的素材目录为空**：`timeless-app/assets/` 无文件，而 `main.rs:25-28` 把资产根指向它；B 要加载的草地贴图 / 18 个 glTF / `NotoSansSC` 全部缺失 | 代码 vs 文件系统 | B 在本检出中跑起来会满屏资产 error |
| C17 | **`architecture.md` 对 B 的多条断言不成立**：domain「有网格 + 时间线结算」（实际只有 `combat.rs`）；「业务逻辑放 `lib.rs` 的 `GamePlugin`」（B 两者皆无）；「应用层不含伤害公式」（B 的 `combat.rs` 有 `damage.div_ceil(2)` 反制伤害） | `architecture.md:7,8,34,36` | 分层文档与两棵树都不完全对得上 |
| C18 | **`app-redesign.md` 与文件系统矛盾**：称 `timeless-domain::combat` 已删除（实际存在，214 行） | `app-redesign.md:15` | 历史文档被当成现状 |
| C19 | **B 的依赖有两个僵尸项**：`icu_segmenter` / `bevy-inspector-egui` 已写进 `Cargo.toml` 但代码 0 引用（egui 面板直接用 `bevy_egui`） | `timeless-app/Cargo.toml:14,16` | 依赖索引失真；`TODO.md` 表里标注的「已引入」需改为「已声明未使用」 |

---

## 五、进度真相（合并口径）

**A（根原型）**：`world` + `voxel_render` + `movement` + `combat` + `timeline`(We-Go 阶段机) + `ai` + `input` + `presentation` + `spawn` 九域全部可编译、有 49 个单元测试；**玩法纵切可跑**（规划 → 提交 → 1s 推进 → 行动落地 → 命中去血 → 重置），但**没有防御 / 资源 / 技能菜单 / 数据驱动数值**。

**B（旧 crate）**：功能上更「像设计稿」（无回合时间线、翻滚 i 帧、招架、火球、精力、egui 面板、中文 HUD、Tab 技能菜单），但**没有** `PendingHit` / `CombatTimeline` / `ActionTemplate` / 三段式前摇窗口；只有 9 个测试，且与 A 不共享任何代码。**更关键的是它在本检出里跑不起来**：素材目录为空、`vendor/parley` 补丁不存在。

**文档侧**：设计层（`timeline.md` / `action-graph.md` / `timeline-core-design.md`）**领先于代码两代**（逻辑刻度时间线 + Action Graph + 延迟命中物化），代码侧**落后但可跑**。差距集中在：逻辑刻度 / `PendingHit` / `ActionTemplate` / 取消特权 / 架势与弹药分线 / AI 意图循环。

---

## 六、统合后的 Backlog（唯一待办清单）

> 每个条目标注适用树（**A** = 根原型 `src/`，**B** = `timeless/`）与验收方式。
> P0 未决 → 不得开工；P1 是让 A 追上 B 的能力；P2 是让 A 追上设计文档。

### P0 — 必须先拍板（阻塞项，先做这 3 件）

- [ ] **D1 选定唯一主线树**（A 或 B），另一棵进入冻结或删除流程。建议：留 **A**（体素 + BSN + 零渲染依赖数据域 + 49 测试，扩展性更好），把 B 的设计能力（无回合时间线 / 防御 / 资源 / 两阶段结算）迁进 A。见第七节。
- [ ] **D2 选定时间线形态**：A 的「规划/推进 + 轮次窗口」还是文档的「无回合持续流动 + 前摇窗口」。这决定 `Phase` / `RoundEnded` / `RESOLUTION_WINDOW` 的存废。
- [ ] **D3 选定坐标模型**：世界空间真实距离（A）还是逻辑刻度网格（文档）。`ecs-combat-components.md` 与 `timeline.md` 必须据此改写。
- [ ] **D4（可选）** 是否保留 `timeless/` 作为对照冻结，还是在本轮一并删除。

### P1 — 让主线树补齐「已设计、已验证」的能力

- [ ] **P1.1 防御与资源**（A）：`Stamina` 组件 + 每轮回复；`Roll`（位移 + `Dodging{i_frames}`）/ `Parry`（`Parrying{target_attack}`）载荷与执行器，走同一条「声明 → 到点执行」通道；`expire_defense` 清理系统。参考 B 的 `combat.rs` / `movement.rs`。
- [ ] **P1.2 火球 / 投射物**（A）：`Fireball` 载荷 + 目标格锁定 + 飞行 + 到达范围伤害（含「范围内无单位则落空」）。B 已有可移植实现。
- [ ] **P1.3 两阶段结算**（A，若 D2 选「保留阶段」）：阶段 1 只读裁决挂 `CombatResult`，阶段 2 统一扣血，消除先手优势。
- [ ] **P1.4 战斗日志与 HUD 补齐**（A）：`HUD` 增加本轮声明之外的信息（命中 / 闪避 / 招架 / 爆炸），日志保留最近 N 行。
- [ ] **P1.5 实机冒烟**（A）：启动无 panic、**无资产加载错误**、规划/推进/重置一轮可玩。把 `TODO.md` 里假的 `[x]` 改回 `[ ]`，通过后填真实结论与日期。（B 侧的同类验收需要先补 `timeless-app/assets/`，见 C16。）

### P2 — 让代码追上设计文档

- [ ] **P2.1 数据驱动动作**：serde + ron 的 `ActionTemplate`（phases / cost / cooldown / hit_frame / impact / damage / range）+ 索引（`usize` / `Handle`，不用 String ID）+ `ActionRegistry` 资源；A 的技能数值（箭 10 伤害 / 横扫 15 / 前摇 0.30 / 0.20）先外置。
- [ ] **P2.2 逻辑刻度时间线**（取决于 D2/D3）：`GlobalTime` 跳跃式推进 + 命中帧校验（基于当时坐标）+ 排序键 `hit_clock → distance → poise → actor`。
- [ ] **P2.3 `PendingHit` 实体化**：弹道 / 延迟 AOE 挂具体实体，可被反制取消；`CombatTimeline` 未来事件堆。
- [ ] **P2.4 取消特权**：`CancelPrivilege` 仅挂玩家 + `CancelRule{ during: Startup, by: [Roll], surcharge, refund_ammo: false }`；前摇 / 判定帧 / 后摇三段式窗口（仅阶段①可取消）。
- [ ] **P2.5 怪物意图循环**（A 的 `ai` 是最大短板）：进入射程 → 预读玩家意图 → 选择攻击 / 走位 / 设防；决策冷却只在后摇结束前不重挑意图。
- [ ] **P2.6 资源分线**：`AmmoPouch` / `Cooldowns` / `Poise` 架势槽；平 A 免费、重击与射击分别吃弹药与精力。
- [ ] **P2.7 信息层（G 层）**：洞察力查看怪物帧 / 射程 / 破势 / 血量；战斗日志回看 N 轮；死亡复盘（谁在哪个刻度命中谁）。

### P3 — A 的表现与工程债

- [ ] **P3.1 单位模型替换**：玩家 / 敌人换真正的角色 glTF（当前是岩石与树占位）。
- [ ] **P3.2 单位贴地与体素碰撞**：`movement` 查 `world` 体素决定是否可走 + 跟随地形爬坡（`TerrainConfig` 已预留说明）。
- [ ] **P3.3 贪婪网格化**：同材质共面合并成矩形，替代逐面四边形（`meshing/utils.rs`）。
- [ ] **P3.4 纹理图集 / AO**：`materials/assets.rs` 换图集 + UV；`lighting` 从面朝向升级为顶点邻域遮挡。
- [ ] **P3.5 区块持久化**：只保存被改动过的区块（`ChunkPinned` + 存档）。
- [ ] **P3.6 方块交互**：放置 / 破坏走 `world::storage::set_voxel`，自动触发重建网格。
- [ ] **P3.7 地表贴图接入**：`textures/ground/grass.png` 目前无人引用。
- [ ] **P3.8 火球 / 命中特效**：Gizmos 或粒子。
- [ ] **P3.9 开发热重载**：启用 `file_watcher`（dev profile）。
- [ ] **P3.10 清理 `assets/LICENSES.md`**：删掉不存在的字体条目，或补回字体资产（若恢复 CJK HUD）。
- [ ] **P3.11 中文 HUD 取舍**：A 现为英文 HUD（默认字体无 CJK）。若要做中文，需自带字体 + 恢复 `vendor/parley` + `icu_segmenter` 方案（B 已有）。

### P4 — 文档收口（随 P0–P3 同步做，不再产出新口径）

- [ ] **P4.1** 按 D1–D3 改写 `ecs-combat-components.md` / `timeline.md` / `game-design.md` / `architecture.md`：在每篇顶部加一行「**描述对象：代码 A / 代码 B / 未来设计稿**」。
- [ ] **P4.2** 重写 `TODO.md`：删除与代码不符的 `[x]`，命令示例按「哪棵树」分开写，测试数改为可核对的口径（A 49 / B 9）。
- [ ] **P4.3** 把 `docs/integration-status.md` 降级为历史（顶部指向本文），或直接删除。
- [ ] **P4.4** 修 `docs/index.md` 索引：补 `app-migration.md` / `app-redesign.md` / `integration-status.md` / `status.md`。
- [ ] **P4.5** 修 `AGENTS.md`：测试数、`docs/status.md` 入口、A/B 两棵树的说明。
- [ ] **P4.6** 修 `timeless/README.md` 与 `app-migration.md` 的过时断言（11 个测试、`AttackStats` 已拆、路径过期）。
- [ ] **P4.7** 修 **代码内注释**（开发者最先看到的一层，优先级高于设计文档）：
  `src/timeline/mod.rs:22` 的 `Space` 声明射击 → `Q` 射击 / `Space` 跳跃；
  `src/timeline/systems.rs:21` 的同句控制台日志；
  `src/lib.rs:13` 与 `src/ai/mod.rs:3-4` 的「AI 只写 `Velocity`」→「AI 只声明行动实体」；
  `src/movement/events.rs:8` 悬空的 `apply_move_command_system` → `declare_move_system`；
  `src/combat/health/events.rs:19` 悬空的 `crate::scene::BattleLog` → `crate::presentation::BattleLog`；
  `src/presentation/hud.rs:176` 注释里的 `—` → `-`（与代码一致）。
- [ ] **P4.8** 定 `BattleLog` 正文语言：要么 HUD 只渲染英文摘要（去掉 `—`/中文正文），要么恢复 CJK 字体资产（P3.11），二选一（当前中文正文会显示成缺字方块）。

---

## 七、需要你拍板的决策（P0）

**D1 主线树：A 还是 B？**

| 选项 | 理由 | 代价 |
| :--- | :--- | :--- |
| **留 A（推荐）** | 领域化模块 + 零渲染依赖数据域（可 `MinimalPlugins` 单测）+ BSN + 体素 + 49 测试；`app-modules.md` 与代码一致；素材齐备（21 glb + 1 png + `LICENSES.md`），**能直接跑**；扩展空间大 | 需要把 B 的无回合时间线 / 防御 / 资源 / 两阶段结算迁过来（P1） |
| 留 B | 设计文档描述的能力大多已在跑（翻滚 i 帧 / 招架 / 火球 / 精力 / egui 面板 / 中文 HUD / Tab 技能菜单），9 测试 | ① `timeless-app/assets/` **为空**，草地 / 模型 / 字体全部缺失，先得补素材；② 网格伪 3D 与体素世界路线冲突；③ 单文件领域、无数据域隔离；④ 文档写的路径（`app/src/...`）与 B 实际路径也不一致；⑤ 9 个测试远少于 A 的 49 |

**D2 时间线：阶段机 vs 无回合？**（A 现状 = 阶段机；文档 = 无回合）
**D3 坐标：世界空间真实距离 vs 逻辑刻度网格？**（A 现状 = 世界空间；`ecs-combat-components.md` = 网格）

三个决策一旦给出，第二节的 P1/P2 排序即可直接执行。

---

## 八、验收命令（按树分开，避免再出歧义）

代码 A（仓库根，package `app`）：

```bash
cargo test                 # 49 个单元测试（src/**）
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo run                  # 体素世界空间纵切
```

代码 B（`timeless/` workspace）：

```bash
cd timeless
cargo test --workspace     # domain 7 + app 2 = 9
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
cargo run -p timeless-app
```

实机冒烟清单（每轮提交前）：启动无 panic · **无资产加载错误**（Bevy 会为缺失 glb 打 error 日志）· 无 ICU4X 刷屏（仅 B 相关）· 规划 → 提交 → 窗口内行动落地 → 扣血 → `R` 重置可复现。

---

## 九、防漂移规则（写文档的人请遵守）

1. **每篇文档顶部必须声明描述对象**：`代码 A（src/）` / `代码 B（timeless/）` / `未来设计稿`。没有这一行，就当作设计稿读。
2. **进度只写进两个文件**：`docs/status.md`（本文，口径与 backlog）与根 `TODO.md`（勾选状态）。其他文档只写「设计意图」，不再写进度。
3. **勾选必须有验收证据**：命令输出、控制台片段或测试名。没有证据的一律保持 `[ ]`。
4. **不引用未落地的标识符**：文档里出现的类型名必须在某棵树的代码中存在（用 grep 核对），或明确标「设计稿」。
5. **改动代码后同步 3 处**：`docs/status.md` 的状态表、`TODO.md` 的勾选、`docs/index.md` 的索引（新增文档时）。
