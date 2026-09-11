# Project Timeless — 项目路线图与依赖索引

> ⚠️ **2026-09 重写**：本文件此前把**两棵并存的代码树**混在一条 Phase 链里，并给未验证的
> 条目打了 `[x]`。现已按实际代码校正：
>
> - **代码 A** = 仓库根 `src/`（package `app`，体素世界空间纵切，**We-Go 规划/推进阶段机**）
> - **代码 B** = `timeless/` workspace（`timeless-app` / `timeless-domain`，网格伪 3D，**无回合**）
>
> 下面每条都标注适用树。**完整差异分析、冲突清单与统合后的 backlog 见
> [docs/status.md](docs/status.md)**；本文件只保留勾选状态、命令与依赖索引。
> 勾选规则：必须有验收证据（命令输出 / 测试名 / 控制台片段）才允许 `[x]`。

## 常用命令（命令按树分开，不要混用）

**代码 A（仓库根目录执行，package `app`）**

```bash
cargo run                    # 启动：体素地形 + 世界空间战斗
cargo test                   # 49 个单元测试（src/**）
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
`voxel_render`（异步面剔除网格化、材质、面朝向明暗）+ `movement`（`Transform` + `Velocity`
位移、移动/跳跃行动）+ `combat`（生命 / 护甲减免 / 碰撞与近战扇形 / 攻击实体生命周期 /
箭矢与横扫技能）+ `timeline`（**We-Go：规划阶段冻结 `Time<Virtual>` 等提交，1s 推进窗口按
`execute_at` 结算，`RoundEnded` 收尾**）+ `ai`（规划阶段声明移动/射击）+ `input`（只翻译）
+ `presentation`（相机 / 装饰 / 英文 HUD / 战斗日志）+ `spawn`（组装车间 + `ResetBattle`）。

**A 没有**：`Position` / `GridMath` 网格坐标、`Stamina` / 弹药 / 冷却、翻滚 / 招架 / 闪避标记、
火球 / 爆炸、两阶段结算与 `CombatResult`、技能菜单（`Can*` / `SKILLS`）、egui 调试面板、
`ActionTemplate` / `PendingHit` / `CombatTimeline`。

详细模块设计见 [docs/design/app-modules.md](docs/design/app-modules.md)（与代码一致）。

## 里程碑

> **Phase 1.5 – 2.2 描述的是代码 B**（那一串条目里的 `GridMath` / `Roll` / `Dodging` /
> `Fireball` / `Parrying` / 中文 HUD 只存在于 `timeless/`）。A 不在这条链上。

### 已完成 ✅（代码 B）

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
  ⚠️ **A 仍然是规划/推进阶段机**，这条不适用于 A。

### Phase 2.0 — 技能与反馈（**仅 B 适用；A 全部未实现**）

- [x] **火球技能**（B）：Tab 循环可选，消耗 2 精力；提交后生成投射物（速度 4 格/秒、
  爆炸 8 伤害/半径 1），目标格在提交时锁定。
- [x] **招架反应**（B）：反应阶段「招架」消耗 1 精力，本回合免疫敌方攻击且不位移。
- [x] **战斗日志 UI**（B）：屏幕右下滚动显示最近战斗消息。
- [x] **精力回复**（B）：每回合结算后回复 1 点（上限 5）。
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

- [x] 防御系统消息链（B：`Dodging` / `Parrying` + 招架执行器与过期清理）。
- [x] 动作实体调度（A：`ScheduledAction` + `Time<Virtual>`；B：`execute_at` + `cast_duration`）。
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
- [ ] 清理 `assets/LICENSES.md` 中不存在的字体条目。
- [ ] 中文 HUD 取舍（A 现为英文 HUD；要 CJK 需自带字体 + `icu_segmenter` 方案）。
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
| serde | 1.0.x | https://crates.io/crates/serde | https://docs.rs/serde/latest | 配置序列化（Phase 2.1，**未引入**） |
| ron | 0.12.x | https://crates.io/crates/ron | https://docs.rs/ron/latest | .ron 配置格式（Phase 2.1，**未引入**） |

引擎官方文档：https://bevy.org/learn/ · 迁移指南：https://bevy.org/learn/migration-guides/

## 验收标准（每次提交前）

**代码 A（仓库根）**

- [ ] `cargo test` 全绿（49）
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
