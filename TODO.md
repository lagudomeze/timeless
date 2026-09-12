# Project Timeless — 项目路线图与依赖索引

> ⚠️ **2026-09 重写**：本文件此前把**两棵并存的代码树**混在一条 Phase 链里，并给未验证的
> 条目打了 `[x]`。现已按实际代码校正：
>
> - **代码 A** = 仓库根 `src/`（package `app`，体素世界空间纵切，**无回合**：`Ready` +
>   每动作前摇/后摇；决策按格、结算按真实距离）
> - **代码 B** = `timeless/` workspace（`timeless-app` / `timeless-domain`，网格伪 3D，
>   **无回合**，已冻结：本检出跑不起来）
>
> 下面每条都标注适用树。**完整差异分析、冲突清单与统合后的 backlog 见
> [docs/status.md](docs/status.md)**；本文件只保留勾选状态、命令与依赖索引。
> 勾选规则：必须有验收证据（命令输出 / 测试名 / 控制台片段）才允许 `[x]`。

## 常用命令（命令按树分开，不要混用）

**代码 A（仓库根目录执行，package `app`）**

```bash
cargo run                    # 启动：体素地形 + 世界空间战斗
cargo test                   # 100 个测试（src/ 下 98 + tests/assets.rs 2），0 跳过
cargo clippy --all-targets -- -D warnings   # 零警告
cargo fmt --check
```

**代码 B（`timeless/` 下执行）**

```bash
cd timeless
cargo run -p timeless-app    # 启动：21×21 网格伪 3D 纵切
cargo test --workspace       # domain 7 + app 2 = 9 个单元测试
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

> 根 `Cargo.toml` **不是** workspace（无 `[workspace]`），所以在仓库根跑
> `cargo test --workspace` 只会跑 A，跑不到 B 的 9 个测试。

## 目录结构

```
根目录
├── AGENTS.md                     # 仓库指南（构建/风格/提交规范）
├── TODO.md                       # 本文件：勾选状态 + 命令 + 依赖索引
├── assets/                       # A 的素材（21 glb + 1 png，LICENSES.md 可追溯）
├── src/                          # 代码 A：app 原型（Bevy 0.19 世界空间纵切，领域化模块）
├── docs/                         # 设计文档
│   ├── status.md                 #   ★ 进度统合 + 文档差异 + 统一 backlog（唯一进度真相）
│   ├── design/                   #   游戏设计 / 时间线 / ECS 组件化 / 架构 / 迁移记录
│   ├── bevy/                     #   Bevy 0.19 速查 / Action Graph
│   ├── art/                      #   素材获取与接入
│   └── index.md                  #   文档索引
├── skills/                       # 可复用 Codex 技能（bevy-019-docs / bevy-assets）
└── timeless/                     # 代码 B：cargo workspace（子项目）
    ├── crates/timeless-domain/   #   领域层：纯 Rust，零 Bevy 依赖
    ├── crates/timeless-app/      #   应用层：Bevy 组件/系统/渲染
    ├── vendor/parley/            #   本地补丁：CJK 分词（README.patch.md）
    └── README.md
```

## 代码 A 现状（根 `src/`，package `app`）

`world`（32³ 体素区块 / 噪声地形 / 体素读写，零渲染依赖，`MinimalPlugins` 可单测）+
`voxel_render`（异步面剔除网格化、材质、面朝向明暗）+ `movement`（`Cell` / `MoveGoal` 格子决策、
`Transform` + `Velocity` 连续位移、移动/跳跃/翻滚行动、投射物飞行）+ `combat`（生命 / 护甲减免 /
碰撞与近战扇形 / 攻击实体生命周期 / 箭矢与横扫 / 火球锁格 + 真实距离 AoE / 精力 / 翻滚无敌帧 /
招架反制 / 两阶段结算 / 技能注册表与菜单）+ `timeline`（**无回合**：`Ready` 决定谁能决策，
`ActionTiming` 决定每个动作的前摇 + 后摇，仅在玩家等输入时冻结 `Time<Virtual>`）
+ `ai`（意图循环 + 声明行动）+ `input`（只翻译）+ `presentation`（相机 / 装饰 / 英文 HUD / 战斗日志）
+ `spawn`（组装车间 + `ResetBattle`）。

**A 仍然没有**（相对 B）：`Position` / `GridMath` 这类第二套网格坐标（A 直接用 `Cell`）、
弹药、egui 调试面板、`ActionTemplate` / `PendingHit` / `CombatTimeline`。

详细模块设计见 [docs/design/app-modules.md](docs/design/app-modules.md)（与代码一致）。

## 里程碑

> **决策已拍板（2026-09）**：保留代码 A 为主线，B 的能力迁进来；时间线改为**无回合**
> （能决策就决策，仅玩家等待输入时冻结）；坐标=**决策按格、结算按真实距离**；
> 节奏=每个动作自带前摇 + 后摇。权威设计见
> [docs/design/timeline-turnless.md](docs/design/timeline-turnless.md)。

### 进行中：无回合重构 + B 能力迁移（A）

- [x] **M1 时间线地基**：`Ready` / `BusyRecovery` / `ActionTiming` / `TimelineConfig`；
      删除 `Phase` / 轮次 / 1s 窗口 / `RoundEnded`；系统链 = 门控 → 提交桥 → 调度 → 后摇恢复。
- [x] **M2 格子移动**：`Cell` / `MoveGoal` / `step_from_axis`；按一次走一格、到格中心吸附停下。
- [x] `F1` 切换 `require_commit`（默认关闭 = 输入直接生效）。
- [x] **M3 资源与防御**：`Stamina`（恢复 `Ready` 时 +1）、翻滚（`F`：1 精力 / 退一格 / 0.5s 无敌帧）、
      招架（`V`：1 精力 / 免伤 + 一半反制）、防御判定插在伤害之前、防御标记过期清理。
- [x] **M4 火球**：`Q` 扔火球 —— 锁目标格、自由飞行、到达后按真实距离结算 12 点 AoE（半径 1.5 格）；
      空地爆炸完全落空；箭矢保留为单体碰撞投射物参考。
- [x] **M5 两阶段结算**：领域层纯逻辑（三层裁决 帧→真实距离→破势、防御判定、反制伤害，8 个单测）
      + `phase1_arbitrate`（只读）→ `phase2_apply`（统一落地）；攻击实体带 `AttackFrame` / `Impact`。
- [x] **M6 AI 意图循环**：选意图与声明行动拆成两个系统；六种意图（含 `Dodge` 威胁预判）；
      威胁用 `CollisionTarget` 判定；翻滚复用玩家的 `RollCommand` 路径。
- [x] **M7 技能菜单与 HUD**：`SKILLS` 注册表（单一来源 + 精力可用性过滤）、
      `MenuSelection` + 选择 / 循环 / 派发（`Attack` 按真实距离派发近战或火球）、
      `1`~`4` / `Tab` / `G` 输入、HUD 技能行与精力。
- [x] **编译 + 测试验证**：`cargo test` **100 通过（98 单元 + 2 资产验收）/ 0 失败 / 0 跳过**；
      `cargo clippy --all-targets` 零警告；`cargo fmt --check` 通过。
- [ ] **收口**：`AGENTS.md` 的按键 / 消息名 / 测试数校正；`ecs-combat-components.md` 按 A 的新组件集改写。

### 三个具名跳过的用例：已全部查清并修复（不再 `#[ignore]`）

根因都不是「火球/翻滚本身有 bug」，而是**测试夹具漏了组件**和**测试写得不稳**：

- [x] `fireball_flies_to_the_locked_cell_and_explodes` —— **夹具漏了 `Stamina`**。
      `declare_fireball_system` 的玩家查询把 `&mut Stamina` 写进了元组，
      少这个组件就整个匹配不到玩家，火球根本没出膛（不是「爆炸没结算」）。
      排查过程：给 `projectile_arrival_system` 打点后发现每帧都跑但 `shells` 为空 →
      用直接 `write_message(FireCommand)` 与 `press(Q)` 对照，确认声明环节失败。
- [x] `roll_spends_stamina_and_grants_invulnerability` —— **观测时机错了**。
      `roll_executor_system` 的 `try_spend` 一直是对的（3 → 2），
      但 `recovery_system` 会在后摇结束时回 1 点精力（`ROLL.recovery = 0.30`），
      而测试在第 6 帧（0.6s）才读，读到的是「扣了又回了」。
      改为在 `Dodging` 出现的那一帧读扣费、跑完后摇再读回复。
- [x] `move_stays_on_the_ground_and_follows_the_camera` —— **断言写错了**。
      `unit_scene` 的起点是地形采样点（格 (1,1) 的**角**），不是格中心，
      所以「角 → 相邻格中心」本来就是一条斜线；位移 `(-1, 0, +1)` 是正确的。
      改为断言真正的不变式：终点 = `step_from_axis(相机前方)` 指出的相邻格中心。

> **Phase 1.5 – 2.2 描述的是代码 B**（那一串条目里的 `GridMath` / `Roll` / `Dodging` /
> `Fireball` / `Parrying` / 中文 HUD 只存在于 `timeless/`）。A 不在这条链上。

### 已完成（代码 B 的历史，留档）

> 以下 Phase 1.5–2.0 全部描述 **B（`timeless/`）**；A 现在通过「无回合重构」重新实现其中
> 的能力（见上面的 M 系列）。B 的素材目录为空、`vendor/parley` 补丁缺失，跑不起来。

- **Phase 0 项目搭建**：workspace 拆分（domain/app）、依赖版本检索。
- **Phase 1 纵向切片**：Message 体系、AI 意图、回合推进、翻滚取消、调试面板、控制台「谁先命中」。
- **Phase 1.5 伪 3D 场景**：21×21 草地地图 + Kenney 装饰、纸片单位（Billboard+贴地阴影）、
  中文 HUD（NotoSansSC + vendor/parley CJK 修复）、右键镜头、Tab 技能切换、WASD 斜向移动、
  Gizmos 移动箭头、悬停格坐标读数。
- **Phase 1.6 模块化重构**：display 拆 mod 目录、移动与战斗分域、
  `AttackStats` → `AttackFrame` / `AttackRange` / `Impact` / `Damage` 小组件（**仅应用层**；
  领域层 `timeless-domain::combat::AttackStats` 仍在用）、意图组件、火球投射物、死亡检查独立系统。
- **Phase 1.7 行动组件化重构**：移除 `Action` 枚举 / `ActionQueue` / `DECISION_OPTIONS`，
  行动载荷组件化（`Attack` / `MoveTo` / `Roll` / `Fireball` / `Parry`）；菜单由能力标记
  （`Can*`）驱动 + `SKILLS` 展示表；防御拆成独立系统，走 `HitPending → dodge → parry → damage`
  消息链。
- **Phase 1.8 坐标与输入系统重构**：删除领域层 `grid.rs`，网格坐标直接用 Bevy `IVec2`；
  `GridMath` 扩展 trait 落在应用层（含单测）；菜单输入拆为消息驱动管线。
- **Phase 1.9 移动领域瘦身**：火球 / 爆炸 / 渲染资源从 `movement.rs` 迁到 `combat.rs`。
- **Phase 1.10 位移速度化**：`Move { target }` → `Velocity(IVec2)`，`step_toward` 删除。
- **Phase 1.11 投射物实体运动化**：位置 + 速度自动飞行，到达目标格后范围内有单位才爆炸。
- **Phase 1.12 动作实体化 + BSN**：载荷组件 + `ScheduledAction` + `Declared → Pending →
  Committed`；`finalize` 分配 `execute_at`，`scheduler` 到期转 `Committed`；两阶段结算
  （阶段 1 只读裁决 + `CombatResult`，阶段 2 统一扣血 + despawn）。
- **Phase 1.13 无回合化重构**（**仅 B**）：删除 `TurnPhase` / `TimeLineState` / `TurnCommitted` /
  `CommitTurn` / 开局暂停；`Time<Virtual>` 默认持续流动，动作实体按 `execute_at` 调度；
  反应改为实时前摇窗口（Q 翻滚取消 / E 招架，不暂停）；`Dodging` 带过期时间。
  ⚠️ **A 当时仍是规划/推进阶段机**，这条不适用于当时的 A；A 在 2026-09 的无回合重构
  （见上面的 M1–M7）之后也变成无回合了。

### Phase 2.0 — 技能与反馈（**仅 B 适用；A 全部未实现**）

- [x] **火球技能**（B，将由 M4 在 A 上重做）
- [x] **招架反应**（B，将由 M3 在 A 上重做）
- [x] **战斗日志 UI**（B；A 有 `BattleLog` 但正文中文会显示成缺字方块，见 status.md C11）
- [x] **精力回复**（B，将由 M3 在 A 上重做）
- [ ] **实机冒烟**：启动无 panic / **无资产错误** / 无 ICU4X 刷屏（B）。
  ⚠️ 本条此前被标为 `[x]` 却无任何验收证据，现改回未完成；A 侧同样未验收。

### Phase 2.1 — 数据驱动动作（A、B 均未开始）

- [ ] serde + ron 加载 `ActionTemplate` 配置（phases / cost / cooldown / hit_frame /
      impact / damage / range）。
- [ ] `ActionId` → 配置索引（`usize` / `Handle`，不用 String ID），应用层不再硬编码技能数值
      （A 当前硬编码：箭 10 伤害 / 横扫 15 / 前摇 0.30 / 0.20）。
- [ ] 技能注册表资源（`ActionRegistry`），HUD / 菜单从注册表读取选项与消耗。

### Phase 2.2 — 逻辑刻度时间线（**设计稿；A、B 均未落地**）

> ⚠️ `docs/design/timeline-core-design.md`（v0.1）与 `docs/design/timeline.md`（v0.2）
> 描述的模型**在两棵树里都没有实现**：无 `GlobalTime` 逻辑刻度跳跃、无 `ExecutionQueue`、
> 无 `PendingHit` / `CombatTimeline`、无 `CancelRule` / `CancelPrivilege` / `try_cancel`、
> 无 `AmmoPouch` / `Cooldowns` / `Poise`、无 `DecisionPause`。

- [x] 防御系统消息链（B 已有；A 待 M3 迁移）。
- [x] 动作实体调度（A：`ScheduledAction` + 固定前摇/后摇；B：`execute_at` + `cast_duration`）。
- [ ] 攻击动作三段式：前摇（可翻滚取消）/ 判定帧 / 后摇，`GlobalTime` 跳跃式推进。
- [ ] `PendingHit` 延迟命中物化（弹道飞行、延迟 AOE 排程），`CombatTimeline` 未来事件堆。
- [ ] 威胁提示改为「前摇窗口」表达（v0.1 的 `DecisionPause` 冻结降级为可选项）。
- [ ] 三层裁决（帧 → 距离 → 破势）：B 的领域层已有纯函数 `resolve_combat`，但**未接入**
      无回合时间线；A 完全没有。

### Phase 3 — 策略深度与多单位

- [ ] 多敌人战斗：单例查询改为多实体查询，AI 每单位独立意图。
- [ ] 范围攻击（L2 排程）、冲刺（位移 2 格）、格挡减伤 + 架势槽。
- [ ] AI 威胁循环：进入射程 → 预读玩家意图 → 设防 / 闪避 / 格挡决策（A 的 `ai` 是最大短板）。
- [ ] 弹药分线（`AmmoPouch`）与技能冷却（`Cooldowns`），资源置换闭环。

### Phase 3.5 — 信息层（G 层：信息即力量）

- [ ] 洞察力：查看怪物数据（帧 / 射程 / 破势 / 血量）、帧窗口细节。
- [ ] 战斗日志回看（历史 N 回合）、死亡复盘（谁在哪个刻度命中了谁）。
- [ ] 成长以知识为主：升级解锁信息权限而非纯数值。

### 代码 A 专属待办（表现 / 工程债）

- [ ] 单位模型替换：玩家 / 敌人当前是岩石与树的占位 glTF。
- [ ] 单位贴地与体素碰撞：`movement` 查 `world` 体素决定可否位移 + 跟随地形爬坡。
- [ ] 贪婪网格化 / 纹理图集 / AO（`voxel_render`）。
- [ ] 区块持久化（只存被改动的区块）+ 方块交互（`set_voxel`）。
- [ ] 接入 `textures/ground/grass.png`（当前无代码引用）。
- [x] 接入 CJK 字体：`assets/fonts/NotoSansSC-Regular.otf`（OFL-1.1），HUD 显式指定它，
      战斗日志的中文不再显示成豆腐块；覆盖由 `tests/assets.rs` 守着。
- [ ] 中文 HUD 文案：字体已就位，但把 HUD 文案翻成中文还需要中文排版
      （断行 / 标点挤压），即 B 用 `vendor/parley` + `icu_segmenter` 解决的那部分。
- [ ] 字体体积：现为 8.3 MB 全覆盖；可子集化成几十 KB（测试不关心体积，只关心覆盖）。
- [ ] 火球 / 命中特效（Gizmos 或粒子）；开发热重载（`file_watcher`）。

## 开发规范（沿袭 AGENTS.md / docs/design/architecture.md）

1. **分层解耦**：`timeless-domain` 绝不引入 Bevy；应用层不含伤害公式。
2. **数据驱动**：数值 / 技能 / 标签反应走外部配置（Phase 2.1，serde+ron），应用层不硬编码。
3. **消息通信**：模块间用 Bevy `Message`；组件 / 消息 / 系统同属一个领域文件（高内聚）。
   UI 输入只翻译成 Message 不直接改状态；输入类消息在插件 `build` 里注册并注明谁写谁消费。
   （A 的注册点在**各领域插件**，不是单一 `main.rs`。）
4. **文档先行**：任何 Bevy API 使用前先查 docs.rs / 官方示例（见 `skills/bevy-019-docs`），
   不依赖训练记忆；本项目固定 Bevy 0.19。
5. **文档防漂移**：每篇设计文档顶部声明「描述对象＝代码 A / 代码 B / 未来设计稿」；
   进度只写 `docs/status.md` 与本文件；引用类型名必须先 grep 确认存在。

## 依赖与文档索引

> 检索方式：本机 crates.io 直连被网络阻断，版本经**清华镜像稀疏索引**查询确认。
> ⚠️ 依赖一律手动写入 `Cargo.toml`（`cargo add` 在镜像下不可用）；新依赖确认版本后先登记下表再引入。

| 依赖 | 版本（最新稳定） | crates.io | docs.rs | 用途 / 引入阶段 |
| :--- | :--- | :--- | :--- | :--- |
| bevy | 0.19.1 | https://crates.io/crates/bevy | https://docs.rs/bevy/0.19.1 | 引擎（A、B 均已引入） |
| bevy_ufbx | 0.19 | https://crates.io/crates/bevy_ufbx | https://docs.rs/bevy_ufbx/0.19 | FBX 加载（仅 A，`rand 0.10.2` 同属 A） |
| bevy_egui | 0.40.1 | https://crates.io/crates/bevy_egui | https://docs.rs/bevy_egui/0.40.1 | 调试面板运行时（仅 B） |
| bevy-inspector-egui | 0.37.0 | https://crates.io/crates/bevy-inspector-egui | https://docs.rs/bevy-inspector-egui/0.37.0 | egui 依赖（仅 B） |
| icu_segmenter | 2.3.0（features=auto） | https://crates.io/crates/icu_segmenter | https://docs.rs/icu_segmenter/2.3.0 | CJK 分词（仅 B，配 vendor/parley 补丁） |
| skrifa | 0.40（**dev-dependency**） | https://crates.io/crates/skrifa | https://docs.rs/skrifa/0.40.0 | 读字体 `cmap` 做字形覆盖验收（`tests/assets.rs`）；版本与 Bevy 依赖树里的 parley 对齐 |
| serde | 1.0.x | https://crates.io/crates/serde | https://docs.rs/serde/latest | 配置序列化（Phase 2.1，**未引入**） |
| ron | 0.12.x | https://crates.io/crates/ron | https://docs.rs/ron/latest | .ron 配置格式（Phase 2.1，**未引入**） |

引擎官方文档：https://bevy.org/learn/ · 迁移指南：https://bevy.org/learn/migration-guides/

## 验收标准（每次提交前）

**代码 A（仓库根）**

- [ ] `cargo test` 全绿（100 = 98 单元 + 2 资产验收，含 0 个 `#[ignore]`）
- [ ] `cargo clippy --all-targets -- -D warnings` 零警告
- [ ] `cargo fmt --check` 通过

**代码 B（`timeless/`）**

- [ ] `cargo test --workspace` 全绿（9）
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 零警告
- [ ] `cargo fmt --check` 通过

**实机冒烟（两棵树各自）**

- [ ] 启动无 panic
- [ ] 无资产加载错误（缺失 glb 会打 error 日志）
- [ ] 一局可玩：声明 → 提交 → 结算 → 扣血 → 重置

> ⚠️ 代码 B 的冒烟目前**无法进行**：`timeless/crates/timeless-app/assets/` 是空目录
> （而 `main.rs:25-28` 把资产根指向它），草地贴图 / 18 个 glTF / `NotoSansSC` 字体全部缺失；
> `timeless/vendor/parley/` 补丁目录也不存在。详见 [docs/status.md](docs/status.md) 的 C15 / C16。
