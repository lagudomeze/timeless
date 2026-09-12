# Project Timeless — 进度与决策记录

> 状态：**唯一进度真相**（最后更新 2026-09-12）
> 维护规则见文末「七、防漂移规则」。
>
> **历史留档**：本文件 2026-09-11 的第一版是一份「文档 vs 代码差异分析报告」，
> 当时的结论是「仓库里有两棵模型相反的树、必须先拍板保留哪一棵」。
> 那部分分析已经完成使命（D1–D5 已拍板并落地），细节搬到
> [design/app-migration.md](design/app-migration.md) 与
> [design/app-redesign.md](design/app-redesign.md) 留档，本文只保留**结论**与**进展**。

---

## 一、两棵代码树的现状

| 代号 | 位置 | 包名 | 状态 | 测试 |
| :--- | :--- | :--- | :--- | ---: |
| **A（主线）** | `src/`（仓库根） | `app` | **无回合**：`Ready` 决定谁能决策，每个动作自带前摇 + 后摇 | **100 通过 / 0 失败 / 0 跳过** |
| **B（冻结）** | `timeless/` | `timeless-app` / `timeless-domain` | 能力已迁入 A；**本检出跑不起来**（`timeless/crates/timeless-app/assets/` 目录不存在、`vendor/parley` 补丁不存在） | 9 |

**D1 已拍板：保留 A，B 的能力迁进来。** 迁移清单与逐项勾选见
根目录 `TODO.md` 的「进行中：无回合重构 + B 能力迁移（A）」。

A 的存活理由留档：领域化模块 + 零渲染依赖数据域（`MinimalPlugins` 可单测）+ BSN
+ 体素世界；素材齐备（21 个 glb + 1 个 png）**能直接跑**。
B 的上位能力（翻滚 i 帧 / 招架反制 / 火球 / 精力 / 技能菜单 / 两阶段结算+三层裁决）
已全部迁入 A；B 仍然领先的只有 `bevy_egui` 调试面板与中文 HUD，两者都在下面的
backlog 里（中文 HUD 的前提是补字体资产）。

## 二、A 的领域与文件（读码确认）

| 领域 | 内容 |
| :--- | :--- |
| `world` | 体素数据域，**零渲染依赖**：`CHUNK_SIZE = 32`、`ChunkLoader`（默认 `radius = (0,1,0)` ⇒ XZ 只加载 1×1 区块、Y 3 层）、噪声地形纯函数 + `TerrainConfig`、`ground_position` 贴地、`MinimalPlugins` 可单测 |
| `voxel_render` | 异步网格化（`AsyncComputeTaskPool` + 句柄挂实体）、面剔除、按类型分组的 `ChunkSurface`、面朝向明暗 |
| `movement` | `Cell` / `MoveGoal` 决策层 + `Transform` / `Velocity` 连续位移；`MoveAction` / `JumpAction` / `RollAction` 载荷与执行器；`move_entities_system` 到格中心吸附 |
| `combat` | `health` / `formula`（纯逻辑 `domain.rs` + 两阶段 `resolution.rs`）/ `attributes` / `targeting` / `lifecycle` / `defense`（精力 + 翻滚 + 招架）/ `skills`（注册表 + 菜单 + 近战 + 箭矢 + 火球 + 爆炸） |
| `timeline` | `ActionTiming`（前摇 + 后摇）/ `ScheduledAction` / `Declared`–`Pending`–`Committed` / `Ready` / `BusyRecovery`；门控 → 提交桥 → 调度 → 后摇恢复 |
| `ai` | `EnemyBrain` + `Intent`；`decide_intent_system`（含威胁预读 → `Dodge`）与 `enemy_declare_system` 拆成两个系统 |
| `input` | 只翻译：键盘 → `MoveCommand` / `JumpCommand` / `FireCommand` / `MeleeCommand` / `RollCommand` / `ParryCommand` / 技能菜单消息 / `ActionsCommitted`；中键 → `PanCamera` |
| `presentation` | 相机（`CameraRig` + `PanCamera`）、装饰（18 种 Kenney glTF 随机摆放）、`BattleLog`（中文正文，见下）、HUD（英文文案 + Noto Sans SC 字体，只读） |
| `spawn` | 组装车间：`unit_scene` + `player.rs` + `enemy.rs` + `assembly.rs` + `restart.rs`（`ResetBattle` 功能胶水） |

按键：`WASD` 移动 · `Q` 火球 · `E` 近战 · `Space` 跳跃 · `F` 翻滚 · `V` 招架 ·
`1`~`4` / `Tab` 选技能 · `G` 释放选中技能 · `Enter` 提交 · `F1` 切换提交模式 ·
`R` 重置 · 中键拖拽平移相机。

## 三、已拍板的决策

**D1 主线树 = A**（根原型 `src/`），B 的能力迁进来。
**D2 时间线 = 无回合**：所有 PC/NPC「能决策就决策」，仅当玩家等待输入时冻结虚拟时间；
默认输入直接生效，另加 `require_commit` 开关（`F1`）要求 Enter 提交。
**D3 坐标 = 决策按格、结算按真实距离**：格（`Cell`，边长 2.0）管决策与同格判定，
命中 / 射程 / 爆炸半径一律用世界距离。
**D4 节奏 = 固定冷却**：每个动作自带前摇 + 后摇，没有冷却计时器。
**D5 投射物 = 锁目标格 + 自由飞行 + 到达后按真实距离结算 AoE**。

D2/D3/D4/D5 已全部落地并通过测试；权威设计文档见
[design/timeline-turnless.md](design/timeline-turnless.md)。

## 四、仍未解决的问题（读码发现，非文档问题）

### 玩法与表现

- [ ] **单位是占位模型**：玩家 = `rock_largeA.glb`、敌人 = `tree_oak.glb`
      （`spawn/player.rs`、`spawn/enemy.rs` 注释自述「后续替换角色模型」）。
- [ ] **单位没有体素碰撞 / 爬坡**：只在生成时贴地（`TerrainConfig` 注释直言），
      因此出生点落在**格角**而不是格中心；第一次移动会顺便把它带到格中心。
- [ ] **没有火球 / 命中特效**：只有实体本身，没有粒子或 Gizmos。
- [ ] **`assets/textures/ground/grass.png` 无人引用**（`grep textures/` 在 `src/` 下 0 命中）。

### 工程债

- [x] **`assets/LICENSES.md` 的字体条目现在是真的**：已补 `assets/fonts/NotoSansSC-Regular.otf`
      （OFL-1.1，8.3 MB），HUD 显式指定它，战斗日志的中文不再显示成豆腐块。
      覆盖由 `tests/assets.rs` 守着（读真实字体查 `cmap`）。体积取舍与子集化出路见
      `assets/LICENSES.md`。
- [ ] **动作数值硬编码**：`timeline::timing` 与 `SKILLS` 的数值应外置成 `.ron`
      （见 [timeline-turnless](design/timeline-turnless.md) 6.6）。
- [ ] **箭矢未接输入**：`ShootAction` / `arrow_scene` 已实现且被测试覆盖，但
      `declare_skill_system` 未注册进插件（避免与火球抢同一条 `FireCommand`），
      计划作为「单体狙击」技能接回。
- [ ] **贪婪网格化**：同材质共面合并成矩形，替代逐面四边形。
- [ ] **纹理图集 / AO**：`materials/assets.rs` 换图集 + UV；`lighting` 从面朝向
      升级为顶点邻域遮挡。
- [ ] **区块持久化**：只保存被改动过的区块（`ChunkPinned` + 存档）。
- [ ] **方块交互**：放置 / 破坏走 `world::storage::set_voxel`，自动触发重建网格。
- [ ] **开发热重载**：启用 `file_watcher`（dev profile）。

### 将来（B 的能力，或设计稿）

- [ ] **egui 调试面板**：B 已有（`bevy_egui` 0.40 + `bevy-inspector-egui` 0.37），A 未接入。
- [ ] **中文 HUD**：**字体这一半已经做了**（`assets/fonts/NotoSansSC-Regular.otf` +
      HUD 显式 `TextFont`），现在中文能正常渲染，战斗日志不再显示成方块。
      仍未做的是**把 HUD 文案本身翻成中文**——这需要中文排版（断行、标点挤压），
      也就是 B 用 `vendor/parley` + `icu_segmenter` 解决的那部分。当前 HUD 文案仍是英文。
- [ ] **`PendingHit` 实体化 / `CombatTimeline` 未来事件堆**：设计稿，未落地。
- [ ] **`ActionTemplate` 资产图**（[bevy/action-graph.md](bevy/action-graph.md)）：
      设计稿；`ActionTiming` 是它的最小落地形态，图遍历与边条件仍未实现。
- [ ] **资源分线**：`AmmoPouch` / `Cooldowns` / `Poise` 架势槽；平 A 免费、
      重击与射击分别吃弹药与精力。当前只有单一 `Stamina`。

## 五、验收命令（按树分开，避免歧义）

代码 A（**仓库根**，package `app`）：

```bash
cargo test                              # 100 通过（98 单元 + 2 资产验收）/ 0 跳过
cargo clippy --all-targets -- -D warnings   # 必须零警告
cargo fmt --check
cargo run                               # 体素世界空间纵切
```

代码 B（`timeless/`，已冻结）：

```bash
cd timeless
cargo test --workspace                  # domain 7 + app 2 = 9
```

> 根 `Cargo.toml` **不是** workspace（无 `[workspace]`），所以在仓库根跑
> `cargo test --workspace` 只会跑 A，跑不到 B 的 9 个测试。

实机冒烟清单（每轮提交前）：启动无 panic · 无资产加载错误（Bevy 会为缺失 glb 打
error 日志）· 决策 → 声明 → 前摇到点落地 → 扣血 → `R` 重置可复现。

## 六、历史分析结论（留档）

第一版 status.md 的核心发现是「两棵树的时间线模型相反，且文档没有声明描述对象」。
那条结论已经解决：A 已改成与 B 一样的无回合模型（只是实现更干净），
且每篇设计文档顶部都声明了描述对象。

仍然值得记住的两条：

1. **B 在本检出中跑不起来**（`assets/` 为空、`vendor/parley` 不存在），
   所以「B 的功能更多」不能作为选型依据，只能作为**能力清单**来迁移。
2. **文档的领先不等于代码的进度**：第一版分析发现 `timeline.md` /
   `ecs-combat-components.md` 描述的 `PendingHit` / `ActionTemplate` / `CancelRule` /
   `AmmoPouch` / `Cooldowns` / `Poise` / 三段式窗口**在两棵树里都不存在**。
   这类内容现在统一标为「设计稿」，且不写进进度口径。

## 七、防漂移规则（写文档的人请遵守）

1. **每篇文档顶部必须声明描述对象**：`代码 A（src/）` / `代码 B（timeless/，冻结）` /
   `未来设计稿`。没有这一行，就当作设计稿读。
2. **进度只写进两个文件**：`docs/status.md`（本文）与根 `TODO.md`（勾选状态）。
   其他文档只写「设计意图」，不再写进度。
3. **勾选必须有验收证据**：命令输出、控制台片段或测试名。没有证据的一律保持 `[ ]`。
4. **不引用未落地的标识符**：文档里出现的类型名必须在某棵树的代码中存在
   （用 grep 核对），或明确标「设计稿」。
5. **改动代码后同步 3 处**：`docs/status.md` 的状态表、`TODO.md` 的勾选、
   `docs/index.md` 的索引（新增文档时）。
6. **不用 `#[ignore]` 隐藏失败**：跳过的用例要么修好，要么在 `TODO.md` 写明根因与
   下一步。`cargo test` 的「N ignored」不是可以接受的常态。
