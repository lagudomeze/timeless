# Project Timeless — 进度与 backlog

> 本文件是**唯一的进度真相**：里程碑勾选状态、待办清单、依赖索引。
> 设计文档只写「是什么、为什么」，见 [`docs/index.md`](docs/index.md)。
> 勾选规则：必须有验收证据（命令输出 / 测试名 / 控制台片段）才允许 `[x]`。

## 验收命令（仓库根目录）

```bash
cargo test                                  # 169 通过（167 单元 + 2 资产验收）/ 0 跳过
cargo clippy --all-targets -- -D warnings   # 零警告
cargo fmt --check
cargo run                                   # 冒烟：体素地形 + 世界空间战斗
```

实机冒烟清单（每轮提交前）：启动无 panic · 无资产加载错误 ·
决策 → 声明 → 前摇到点落地 → 扣血 → `F5` 重置可复现。

## 当前代码是什么

`src/`（package `app`）= 10 个领域 + 组装车间：

- `world` 32³ 体素区块 / 噪声地形 / 体素读写（零渲染依赖，`MinimalPlugins` 可单测）
- `voxel_render` 异步面剔除网格化 / 按类型分组材质 / 面朝向明暗
- `movement` `Cell` + `MoveGoal` 格子决策、`Transform` + `Velocity` 连续位移、
  移动 / 跳跃 / 翻滚载荷与执行器
- `combat` 生命 / 护甲公式 / 碰撞与近战扇形 / 攻击实体生命周期 / 箭矢与横扫 /
  火球锁格 + 真实距离 AoE / 精力 / 翻滚无敌帧 / 招架反制 / 威胁检测（反应系统）/
  `SKILLS` 注册表与技能菜单
- `timeline` **无回合**调度：`DecisionSlot` 决定谁能决策，`ScheduledAction` 的时间戳
  决定前摇 + 后摇，`PauseReasons` 决定什么时候冻结 `Time<Virtual>`（唯一写时钟的是
  帧末的 `apply_clock`）；`Focus` 让玩家把一次前摇买掉
- `ai` 六种意图（含威胁预判）+ 声明行动
- `input` 只翻译 · `interaction` 鼠标拾取 / 高亮 / 预演 / 点击解释
- `presentation` 相机 / 单位纸片与贴地阴影 / 装饰 / 中文日志 / 英文 HUD
- `spawn` 组装车间 + `ResetBattle`

模块地图、流水线顺序、按键表见 [`docs/architecture.md`](docs/architecture.md)；
组件与系统的逐层对照见 [`docs/components.md`](docs/components.md)。

## 已完成

### 无回合重构 + 能力迁移（M1–M15）

- [x] **M1 时间线地基**：`Ready` / `BusyRecovery` / `ActionTiming`；
      删除 `Phase` / 轮次 / 1s 窗口 / `RoundEnded`；系统链 = 门控 → 提交桥 →
      调度 → 后摇恢复。
- [x] **M2 格子移动**：`Cell` / `MoveGoal` / `step_from_axis`；按一次走一格、
      到格中心吸附停下。
- [x] **M3 资源与防御**：`Stamina`（恢复 `Ready` 时 +1）、翻滚
      （1 精力 / 退一格 / 0.5s 无敌帧）、招架（1 精力 / 免伤 + 一半反制）、
      防御判定插在伤害之前、防御标记过期清理。
- [x] **M4 火球**：锁目标格、自由飞行、到达后按真实距离结算 12 点 AoE
      （半径 1.5 格）；空地爆炸完全落空。
- [x] **M5 两阶段结算**：领域层纯逻辑（三层裁决帧 → 真实距离 → 破势、
      防御判定、反制伤害）+ `phase1_arbitrate`（只读）→ `phase2_apply`（统一落地）。
- [x] **M6 AI 意图循环**：选意图与声明行动拆成两个系统；六种意图（含 `Dodge`）；
      威胁用 `CollisionTarget` 判定。
- [x] **M7 技能菜单与 HUD**：`SKILLS` 注册表（单一来源 + 精力可用性过滤）、
      `MenuSelection` + 选择 / 循环 / 派发（`Attack` 按真实距离派发近战或火球）。
- [x] **M8 单位 2D 纸片 + 贴地阴影**：billboard 绕 Y 轴对准相机，
      高度用正下方地表上的黑色阴影表示（Kenney Tiny Dungeon，CC0）。
- [x] **M9 HUD 重构**：五块布局（时间轴 / 双方面板 / 技能栏 / 可折叠日志 / 帮助）；
      `UiScale` 按窗口高度适配；`HudCache` 快照比对（内容没变就整帧不碰 UI）。
- [x] **M9.2 移动手感**：新增 `follow_terrain_system`（贴地）与
      `end_action_until`（忙到真正到达目标格）；时间轴色块从 `declared_at` 长出去。
- [x] **M9.3 以 PC 为中心**：出生点用格中心（`spawn::cell_ground`）、
      相机跟随 PC（中键拖拽 = 观察偏移）、`ActionBlocked` 提示条。
- [x] **M9.4 地形量化到决策格**：`TERRAIN_CELL = 2`，整格 2×2 体素共享一个高度，
      台阶只出现在格边界。
- [x] **M9.5 时间轴分道 + 候场区**：每个单位一行、右侧显示「已就绪还没声明」的人。
- [x] **M9.6 AI 复活 + 行动归属**：`EnemyBrain` 用 `#[require(Intent)]`；
      AI 的 `Dodge` 直接生成自己的 roll 行动，不借玩家的 `RollCommand`。
- [x] **M10 鼠标 Raycast + 悬停高亮**：`interaction` 新域，沿地形高度场步进的纯函数拾取。
- [x] **M11 点击交互**：左键点地板走 / 点单位用技能、右键撤销；
      火球改成执行时才发射（撤销不留半空火球）。
- [x] **M12 预演指示器**：火球 AOE 圆盘 + 近战扇形，判据与派发系统一致。
- [x] **M13 预演读数 + 滚轮缩放**：提示条显示「技能 · 格 · 距离 · 预计伤害」；
      滚轮改 `CameraRig.zoom`。
- [x] **M14 打断 / 撤销 / 取消代价**：`WASD` 让给技能热键、数字键改成直接放技能、
      `F2` 循环反应窗口（`Loose` / `Strict` / `Off`）、`interrupt_system`、
      `ActionCost` / `CancelCost` / `Uncancellable`。
- [x] **M15 同帧阵亡崩溃 + 远射被冻在半路**：收尾全程走 `Commands::get_entity` 守卫；
      火球用 `end_action_until(.., flight_time)` 忙到落地。
- [x] **M16 时间戳 + 事件模型（本次重构）**：删掉 `Ready` / `BusyRecovery` /
      `Declared` / `Pending` / `Committed` / `ActionCost` / `CancelCost` /
      `Uncancellable` / `Impact` 与 `Arbitration` 两阶段结算，改为
      「`DecisionSlot` + `ScheduledAction.execute_at` + `Busy` + `Cancellable`」：
      执行器自己判 `due()`、自己销毁行动实体、自己挂 `Busy`（无 `scheduler_system`、
      无 `begin_action` / `end_action`）。暂停改成**原因集合**（`"manual"` /
      `"slot_empty"` / `"threat"`，唯一的时钟写入点是 `apply_clock`，空格不再只前进一帧）；
      新增**反应系统**（`Threatens` / `TargetCell` → `detect_threat_system` → 冻结等玩家表态）
      与 **Focus**（Shift + 决策键：扣 1 点把前摇归零）；打断改成
      `InterruptEvent`（EntityEvent + Observer，掷骰对抗打掉**还没到点**的行动）；
      伤害压成一条链（`DamageEvent` → `apply_damage_system` → `DeathEvent` →
      `despawn_dead_system`，血量改整数、扣到负数继续扣、死亡只报一次）。
      验收：169 测试全绿 / clippy 零警告 / `rg "ResMut<Time<Virtual>>" src` 只命中 `apply_clock`。
- [x] **CJK 字体**：`assets/fonts/NotoSansSC-Regular.otf`（OFL-1.1），
      HUD 显式指定，战斗日志中文不再显示成豆腐块。
- [x] **文档整合**：文档收敛为「入口 + 架构 + 时间线 + 组件对照 + 设计 + 素材 +
      Bevy 速查」七篇，进度合并进本文件；移除已冻结的 `timeless/` workspace。

## 待办

### 工程债（按优先级）

- [ ] **动作数值外置**：`timeline::timing` 与 `SKILLS` 的数值改成 `.ron`
      （serde + ron），应用层不再硬编码技能数值；顺带做 `ActionRegistry` 资源，
      让 HUD / 菜单从注册表读选项与消耗。
- [x] **死代码清理**：删除 `defense/actions.rs` 里重复且未注册的
      `expire_defense_markers_system`、`lifecycle::manage_projectile_hits_system`、
      写而无消费的 `AttackResolved`。剩下的 `declare_skill_system` / `arrow_scene`
      是「单体狙击」的预留实现（收尾已与火球对齐），接输入即可用。
- [x] **`Space` 手动暂停只前进一帧**：改成暂停原因集合，手动暂停一直有效直到再按一次
      （见 M16）。
- [x] **箭矢的收尾方式**：`shoot_action_executor_system` 现在忙到
      `距离 / ARROW_SPEED`（与火球共用 `Busy::after`）。
- [x] **玩家身份收口**：新增 `timeline::InputDriven` 标记（组装层挂在玩家身上），
      时间线 / 反应系统 / 撤销 / 各玩家声明系统都认它，不再满世界找 `Faction::Player`。
      表现层仍按 `Faction` 找单位——它看的是阵营，不是输入归属。
- [ ] **反应窗口的粒度**：现在是"一次威胁一个窗口"，多段攻击（连续三刀）只会问玩家一次。
      将来按威胁**来源**分别开窗（`ThreatWindow` 存集合而不是一个 `Option`）。
- [ ] **AI 不会用 Focus**：`Focus` 只在玩家侧，敌人声明固定排前摇。
      要让精英怪也会抢先手，得给 AI 一套"什么时候值得花资源"的策略。
- [ ] **`AttackFrame` 没有消费者**：攻击实体都挂着它，但没有系统读——
      它是「洞察力」面板的读数（"谁先动"），接上或删掉。
- [ ] **没有生产者的预留类型**：`Voxel` / `VoxelPos` / `ChunkPinned`——接上或删掉。
- [ ] **开发热重载**：启用 `file_watcher`（dev profile）。

### 玩法与表现

- [ ] **箭矢接回输入**：`ShootAction` / `arrow_scene` 已实现且被测试覆盖，
      但 `declare_skill_system` 未注册（避免与火球抢同一条 `FireCommand`），
      计划作为「单体狙击」技能接回。
- [ ] **单位贴地与体素碰撞**：`movement` 查询 `world` 的体素决定目标格是否可走
      （当前点击目标格走直线，没有可行走性判定与寻路）。
- [ ] **火球 / 命中特效**：目前只有实体本身，没有粒子或 Gizmos。
- [ ] **护甲接进组装层**：公式支持 `Armor`，但还没有任何单位挂它。
- [ ] **接入 `assets/textures/ground/grass.png`**（当前无代码引用）。

### 渲染优化（`voxel_render`）

- [ ] **贪婪网格化**：同材质共面合并成矩形，替代逐面四边形。
- [ ] **纹理图集 / UV**：`materials/assets.rs` 换图集，网格化代码不动。
- [ ] **AO**：`lighting` 从面朝向明暗升级为按顶点的邻域遮挡。
- [ ] **区块持久化**：只保存被改动的区块（`ChunkPinned` + 存档）。
- [ ] **方块交互**：放置 / 破坏走 `world::storage::set_voxel`，自动触发重建网格。

### 字体与文本

- [ ] **字体覆盖验收**：`tests/assets.rs` 目前只验「文件在、是合法 sfnt」；
      逐字查 `cmap` 要引第三方库（`skrifa`，已移除）。release 前评估是否加回。
- [ ] **CJK 断行**：Bevy 文本栈缺 `icu_segmenter` 的 CJK 分词模型，运行时会打印
      `ICU4X data error: No segmentation model for complex script`
      （正文仍正常渲染，只是断行退化）。
- [ ] **中文 HUD 文案**：字体已就位，把 HUD 文案翻成中文还需要中文排版
      （断行 / 标点挤压）。
- [ ] **字体体积**：现为 8.3 MB 全覆盖，可子集化到几十 KB。

### 策略深度（目标形态，未落地）

- [ ] **多敌人战斗**：单例查询改多实体查询，AI 每单位独立意图与威胁排序。
- [ ] **资源分线**：`AmmoPouch`（重击 / 射击）/ 架势槽 `Poise`（打断抗性 / 格挡）；
      平 A 免费。
- [ ] **格挡减伤**：与现有翻滚 / 招架并列的第三条防御路径。
- [ ] **范围攻击排程 / 冲刺（位移 2 格）**。
- [ ] **`ActionTemplate` 资产图**：动作的静态定义（相位 / 消耗 / 效果 / 可取消规则）
      资产化，按边条件在图上转移。当前已落地的最小形态是
      `ActionTiming { windup, recovery }` + 行动实体。
- [ ] **延迟命中实体化（`PendingHit`）**：把延迟 AOE / 地面效果排到未来事件堆上。

### 信息层（G 层：信息即力量）

- [ ] **洞察力**：查看怪物数据（帧 / 射程 / 打断抗性 / 血量）、帧窗口细节。
- [ ] **战斗日志回看**（历史 N 条）与**死亡复盘**（谁在哪个时刻命中了谁）。
- [ ] 成长以知识为主：升级解锁信息权限而非纯数值。

## 依赖与文档索引

> 本机 crates.io 直连不可用，版本经**清华镜像稀疏索引**查询确认。
> 依赖一律手动写入 `Cargo.toml`（`cargo add` 在镜像下不可用）；
> 新依赖确认版本后先登记下表再引入。

| 依赖 | 版本 | 用途 |
| :--- | :--- | :--- |
| bevy | 0.19.1 | 引擎（`Cargo.toml` 写 `0.19`，`Cargo.lock` 锁 0.19.1） |
| rand | 0.10.2 | 装饰物随机摆放 |
| bevy_brp_extras | 0.22 | 运行时调试协议扩展：截图 / 输入模拟 / 干净退出 |
| serde + ron | 未引入 | 配置序列化（「动作数值外置」时引入） |

引擎官方文档：<https://bevy.org/learn/> ·
迁移指南：<https://bevy.org/learn/migration-guides/> ·
符号检索：<https://docs.rs/bevy/latest/bevy/?search=>

## 开发规范（摘要）

1. **分层解耦**：`combat/formula/domain.rs` 零 Bevy 依赖；应用层不含伤害公式。
2. **数据驱动**：数值 / 技能走外部配置（见「动作数值外置」），应用层不硬编码。
3. **消息通信**：模块间用 Bevy `Message`；组件 / 消息 / 系统同属一个领域文件；
   UI 输入只翻译成消息；消息在**消费方**插件注册并注明谁写谁消费。
4. **文档先行**：用 Bevy API 前先查 docs.rs / 官方示例（`bevy-019-docs` skill），
   不依赖训练记忆。
5. **文档防漂移**：进度只写本文件；文档里引用的类型名必须先在 `src/` 里 grep 确认存在。

完整规范见 [`AGENTS.md`](AGENTS.md)。
