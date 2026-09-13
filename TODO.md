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
cargo test                   # 154 个测试（src/ 下 152 + tests/assets.rs 2），0 跳过
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
- [x] `F2` 切换 `require_commit`（默认关闭 = 输入直接生效；`F1` 后来让给了帮助面板）。
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
- [x] **M8 单位 2D 纸片 + 贴地阴影**：玩家 / 敌人从 glTF 占位换成 2D 精灵
      （`presentation/unit_sprite.rs`：billboard 绕 Y 轴对准相机），高度用**正下方地表上的
      黑色阴影**表示（离地越高、阴影越小）；素材 Kenney Tiny Dungeon（CC0，见
      `assets/LICENSES.md`）。验收：`cargo test` **106 通过（103 单元 + 3 资产验收）/
      0 失败 / 0 跳过**、`cargo clippy --all-targets -- -D warnings` 零警告、
      `cargo fmt --check` 通过；手工截图确认「玩家抬到离地 1.5 格时纸片升空、阴影留在地面并收缩」。
- [x] **M9 HUD 重构**：左上角那坨纯文本拆成五块——顶部时间轴（行动色块）、
      左下玩家 / 右下敌人面板（头像 + HP / EN 条 + 状态行 + 当前行动）、底部居中技能栏
      （图标 + 消耗角标 + 悬停 tooltip）、右下偏上可折叠战斗日志、`F1` 帮助面板；
      常驻按键提示撤掉；`UiScale` 按窗口高度适配分辨率。`F1` 让给帮助面板，
      提交模式开关挪到 `F2`；HUD 走**快照比对**（`HudCache`：内容没变就整帧不碰 UI，
      WeGo 冻结时几乎不产生 UI 写入）。验收：`cargo test` **123 通过（120 单元 + 3 资产验收）/
      0 失败 / 0 跳过**、`cargo clippy --all-targets -- -D warnings` 零警告、
      `cargo fmt --check` 通过；手工截图确认五块布局、待执行行动的时间轴色块与帮助面板。
- [x] **M9.1 HUD 可诊断性 + 时间轴细节**（BRP 复查发现的问题）：HUD 每个节点挂 `Name`
      （`PlayerHpFill` / `SkillSlot2Badge` / `TimelineBlock3` …），HUD 标记组件
      （`PanelBar` / `PanelText` / `SkillSlot` / `TimelineBlock` / `LogPanel` / `HelpPanel` …）
      在 `PresentationPlugin` 里 `register_type`，BRP 的 `world.query` 与按名截图因此能直接
      定位 UI 实体；时间轴隐藏色块归零 `left`/`width`（防幽灵色块）、「现在」刻线用
      `ZIndex(1)` 压在色块之上、色块池改用一次 `add_children` 固定 slot 0→7 的 z 序。
      顺带发现工作区误删了 `skrifa` dev-dependency（`tests/assets.rs` 的字体 cmap 验收要用，
      缺它 `cargo test` 编译不过）；经 BRP 实机确认中文正文渲染无豆腐块后，按决定**移除**了
      该 cmap 验收与 `skrifa` 依赖（字体覆盖改为运行时人工确认，release 前再评估）。
      验收：`cargo test` **125 通过（123 单元 + 2 资产验收）/ 0 失败 / 0 跳过**、
      `cargo clippy --all-targets -- -D warnings` 零警告、`cargo fmt --check` 通过；
      BRP 实机复验：按名查询/截图、`PanelBar` 反射、色块池顺序、`ZIndex` 刻线、
      EN 条按 2/5 收缩、敌人阵亡后面板显示 `down`、技能 tooltip、日志中文正文与折叠、F1 帮助面板。
- [x] **M9.2 移动手感 + 时间轴可读性**（BRP 实机复验发现的两个手感 bug 与时间轴语义）：
      **① 贴地**：新增 `movement::follow_terrain_system`（有 `Cell` 且不在 `Jumping` 的单位，
      `y` 按 `ground_position` 追平地表，限速 15/s；到位那一帧由 `move_entities_system`
      精确吸附，避免冻结在"爬到一半"）；**② 忙到到位**：新增
      `timeline::end_action_until`，移动按「距离 / 速度」把 `ready_at` 推到真正到达目标格，
      修掉"后摇 0.10s < 走一格 0.40s → 滑行途中可决策 → 用旧 `Cell` 当起点 → 反向掉头 / 回弹"；
      **③ 时间轴**：色块左边界从 `execute_at` 改成 `declared_at`（块从"现在"刻线长出去），
      块内加白色**结算刻线**（`windup / total`），轨道加每 0.5s 的**秒刻度**；
      **④ 预演冻结**：`timeline_gate_system` 把"存在草案"也算作等玩家决定——以前草案
      一声明就摘掉 `Ready`，世界不停表，草案的时间窗会在玩家犹豫时溜走（色块消失、敌人照常行动），
      预演因此白做。
      验收：`cargo test` **131 通过（129 单元 + 2 资产验收）/ 0 失败 / 0 跳过**、
      `cargo clippy --all-targets -- -D warnings` 零警告、`cargo fmt --check` 通过；
      BRP 实机复验：Draft 色块 `left=0.0% / width=6.25% / α=0.5`、结算刻线在块内 60%、
      7 条秒刻度、状态行 `draft ready (Enter to commit)`、提交后玩家 `y` 从 `0.0` 跟到 `-1.0`
      （与同位置装饰物的地表高度一致）。
- [x] **M9.3 以 PC 为中心 + 「无法操作」反馈**：**① 出生点居中**（新增
      `spawn::cell_ground`，玩家 / 敌人都用**格中心**出生：`PLAYER_SPAWN = (1,0)`、
      `ENEMY_SPAWN = (3,3)`——以前玩家落在格 (1,1) 的**角**上，第一步会走成斜线）；
      **② 镜头跟随 PC**（`camera_follow_system`：`focus` 指数收敛到「玩家脚下 + 观察偏移」，
      首帧直接吸附、用 `Time<Real>` 所以冻结时也能追完；中键拖拽改为拉 `pan_offset`，
      上限 `CameraRig::PAN_RADIUS = 8`，既看得见四周又不会把 PC 甩出画面）；
      **③ 「无法操作」提示**（`ActionBlocked` 消息住在 `timeline`——它讲的是「谁能决策」，
      避免 movement/combat 反向依赖 presentation；6 个声明系统在玩家没 `Ready` 或精力不够时
      写消息，HUD 新增 `hud/hint.rs` 弹出 2s 的短句 "CAN'T ACT YET · still busy" /
      "NOT ENOUGH ENERGY"，计时走真实时间所以冻结时也能淡出）。
      验收：`cargo test` **136 通过（134 单元 + 2 资产验收）/ 0 失败 / 0 跳过**、
      `cargo clippy --all-targets -- -D warnings` 零警告、`cargo fmt --check` 通过；
      BRP 实机复验：玩家出生世界 `(3,-1,1)` = 格 (1,0) 中心、敌人 `(7,-1,7)` = 格 (3,3) 中心、
      镜头始终把 PC 放在画面中心、连续输入被拒时左下角出现提示条。
- [x] **M9.4 地形量化到决策格**（修的 bug：树"长在地面下"、人"和地面重叠"）：
      根因是**地形粒度与决策粒度不一致**——地形按**单个体素列**（1 单位）起伏，而
      `Cell` 边长 `CELL_SIZE = 2`，单位与装饰都摆在**格中心**（世界坐标正好落在体素
      **边界**上）；footprint 稍大的对象（0.5~1.7 格宽的树、1.5 格宽的纸片）于是有一半
      落进邻居方块里，看上去就是陷进地面。
      修法：`world::terrain` 新增 `TERRAIN_CELL = 2`，`surface_height` 的采样点改取
      **所在格的中心**（`div_euclid` 量化），整格 2×2 体素共享一个高度；台阶只出现在
      格与格的边界上。两条测试守着：`terrain_quantisation_matches_the_decision_cell_size`
      （与 `timeline::CELL_SIZE` 必须相等）与 `every_voxel_column_inside_a_cell_shares_one_height`。
      副作用：地形看起来变成 2×2 的整齐平台，噪声特征在**格**尺度上大一倍
      （`TerrainConfig::scale = 6.0` 现在是 6 格；想恢复原来的疏密把它调到 3.0 即可）。
      验收：`cargo test` **136 通过（134 单元 + 2 资产验收）/ 0 失败 / 0 跳过**、
      `cargo clippy --all-targets -- -D warnings` 零警告、`cargo fmt --check` 通过；
      BRP 实机截图确认玩家纸片整体可见、不再与地面重叠。
- [x] **M9.5 时间轴分道 + 候场区**（"一条横轴看不出谁在动手"）：时间轴改成
      `Timeline → [行首字母列 | 车道列 | 候场区]`——
      **① 每个单位一行**（`LANE_POOL = 4`，玩家永远在最上面一行，其余按实体序号稳定排），
      每条车道有自己的色块池（`BLOCK_POOL_PER_LANE = 2`）与行首字母（`P` / `E`，用阵营色）；
      两个单位同时出手不再互相叠压。**② 右侧候场区**只显示"已就绪、还没声明"的人
      （`TimelineReadyChip`）：声明后离开候场、色块进入自己的车道，一眼能看出还剩谁没动。
      **③ 秒刻度与"现在"刻线横跨所有车道**（挂在车道容器上、高度 100%，刻线 `ZIndex(1)`
      保证压在最上层）。查询之间用 `Without` 两两互斥，并抽成 `StateTextQuery` /
      `LaneLabelQuery` 等别名（既躲 B0001 参数冲突，也躲 clippy 的 `type_complexity`）。
      `F1` 帮助面板补了 3 行读法（每单位一行 / 块内竖线 = 结算时刻 / 右侧候场 = 还没决定）。
      验收：`cargo test` **137 通过（135 单元 + 2 资产验收）/ 0 失败 / 0 跳过**、
      `cargo clippy --all-targets -- -D warnings` 零警告、`cargo fmt --check` 通过；
      BRP 实机复验：两人都就绪时候场区并排显示蓝 `P` / 红 `E`；`F2` + `W` 声明草案后
      玩家车道出现半透明色块（左边界 0%、块内白色结算刻线），候场区只剩红 `E`，
      `TimelineReady0` 的 `Node.display` 变 `None`。
- [x] **M9.6 AI 复活 + 行动归属**（按「只有 PC 靠按键决策」的约定修）：
      **① AI 从来没跑起来**：`Intent` 只在测试里被插过，`enemy_scene` 与 `EnemyBrain`
      都没带它，而 `decide_intent_system` / `enemy_declare_system` 都要求 `&mut Intent`
      —— 两个系统静默地一个都不匹配，敌人全程站着不动（`HEAD` 里就是这样，不是这次重构
      引入的）。修法：`EnemyBrain` 加 **`#[require(Intent)]`**，组装层从此不可能再漏；
      两个 AI 组件同时进反射，BRP 可直接读 `app::ai::components::Intent`。
      **② `RollCommand` 归属**：它是**玩家输入消息**，但 AI 的 `Intent::Dodge` 也写它，
      而 `declare_roll_system` 会遍历所有就绪单位 → 玩家按 F 会把就绪的敌人一起带着滚。
      按约定改成：AI 直接调 `defense::declare_roll(...)`（载荷工厂 + `begin_action`，
      **行动实体的形状完全一样，区别只在触发源**），`RollCommand` 只留给 PC；
      `roll_step()` 提升为纯函数供两边共用。
      验收：`cargo test` **139 通过（137 单元 + 2 资产验收）/ 0 失败 / 0 跳过**、
      `cargo clippy --all-targets -- -D warnings` 零警告、`cargo fmt --check` 通过；
      新增测试 `a_dodging_enemy_declares_its_own_roll`（AI 自己生成 roll，且不碰玩家的
      `Ready`）与 `the_players_roll_command_only_moves_the_player`；BRP 实机：敌人
      `Intent = Approach`、位置从出生 `(7,-1,7)` 走到 `(7,-1,5)` 贴到玩家身边，面板显示
      `busy · cell (2,2) · dist 2.0 · approach`，此时候场区只剩蓝 `P`（敌人在忙）。
- [x] **M10 第二阶段①鼠标 Raycast + 悬停高亮**（`src/interaction/` 新域）：
      **① 拾取**：`cursor_ray`（`Camera::viewport_to_world`，0.19 的签名）+
      `pick_cell`（沿**地形高度场**以 1/4 格步进，第一次钻到地表以下即命中）——
      纯函数、不读 `ChunkMap`、不受区块加载影响，单测里手搓射线即可验；探测上限
      `MAX_PICK_DISTANCE = 160`，超时/指天空返回 `None`。**② 状态**：`HoveredCell(Option<Cell>)`
      资源（注册进反射，BRP 可直接读"现在指着哪一格"）。**③ 高亮**：开局生成唯一的
      `HoverHighlight` 平面（`NotShadowCaster`、unlit、α0.35），每帧搬到悬停格地表 +0.03；
      颜色按**踩到了谁**走（空地青 / 自己蓝 / 敌人红），当前色进 `HoverTint` 组件而不是
      只藏在材质里——排查"高亮没出现"时先读 `HoveredCell`，一眼分辨拾取问题还是画面问题。
      新域排在流水线 `InputSet → InteractionSet → TimelineSet`；`Cell` 也补了反射。
      验收：`cargo test` **146 通过（144 单元 + 2 资产验收）/ 0 失败 / 0 跳过**、
      `cargo clippy --all-targets -- -D warnings` 零警告、`cargo fmt --check` 通过；
      BRP 实机复验：光标移到 (700,420) → `world.get_resources` 读 `HoveredCell` =
      `{x:2, z:0}`，高亮实体 `Visibility=Visible`、`HoverTint=青`、位置 `(5,-0.97,1)`
      = 格 (2,0) 中心 + 抬升，截图可见青色半透明方块。
- [x] **M11 第二阶段③点击交互**（左键动手、右键撤销、默认预演态）：
      **① 输入**：`input::pointer_click_input_system` 只把左右键翻成 `PointerCommand`
      （写方是 input，消息定义在 interaction）；`pointer_command_system` 再解释成
      **各领域的消息**——左键点空地板 → `MoveToCommand`（可跨多格直线走）、点单位 →
      `UseSelectedSkill { target_cell }`（按当前选中技能）；右键 → `UndoCommand`。
      **② 撤销**：`timeline::undo_system`
      销毁玩家那条尚未结算的行动（`Declared`/`Pending`，`Committed` 撤不掉）、恢复
      `Ready`、清草案，并广播 `ActionCancelled { actor, refund }` 让
      `combat::defense::refund_cancelled_actions_system` 退还精力（退款额来自行动实体上的
      `ActionCost`——**时间线不认识载荷，也不认识资源**）。**③ 顺带修正火球语义**：
      投射物改成**执行时才发射**（`FireballAction { target_cell }`），
      以前声明时就扔出去，撤销会留下飞行中的火球。
      验收：`cargo test` **151 通过（149 单元 + 2 资产验收）/ 0 失败 / 0 跳过**
      （新增：左键两种去向、右键撤销、跨多格移动、撤销退款）、
      `cargo clippy --all-targets -- -D warnings` 零警告、`cargo fmt --check` 通过。
      ⚠️ 当时的结论是「BRP 注入的鼠标按键送不进应用」——**已推翻**：后来的排查里
      `brp_extras_click_mouse` 是通的（左键点击让玩家从 (1,0) 走到 (1,3)），
      当年那次失败是**应用没拿到焦点**，不是代码问题。
      > 注：本条的「默认预演态」`require_commit = true` 后来被 **M14** 取代——
      > 用户拍板**不要"确认"这一步**：声明即生效，反悔靠右键 / 新意图打断。
- [x] **M12 第二阶段④预演指示器**：`interaction` 新增两个地面指示器——
      **火球 AOE 圆盘**（`AoePreview`，半径 `FIREBALL_RADIUS`、红 α0.22，落在悬停格地表）
      与**近战扇形**（`ConePreview`，`CircularSector` 半径 `MELEE_REACH`、张角 120°、琥珀 α0.22，
      贴在玩家脚边并朝悬停格转）。显示规则与 `use_selected_skill_system` 同一判据：
      选火球 → AOE；选近战 → 扇形；选「攻击」按悬停距离二选一（贴脸扇形 / 远了圆盘）；
      选翻滚 → 都不显示。`MELEE_REACH` 从 `menu` 升为 `pub`（和 AI 同一约定，不再各写一份）。
      验收：`cargo test` **152 通过（150 单元 + 2 资产验收）/ 0 失败 / 0 跳过**
      （新增"选不同技能显示不同指示器 + AOE 落在悬停格"）、`cargo clippy --all-targets
      -- -D warnings` 零警告、`cargo fmt --check` 通过；BRP 实机：悬停远处格时
      `AoePreview` 为 `Visible`、位置 `(9,-0.97,1)` = 格 (4,0) 中心 + 抬升，
      `ConePreview` 为 `Hidden`（截图 `v11_aoe_preview.png` 可见红色半透明圆盘）。
      ⏳ 还差最后一件：HUD 上的「距离 / 预计伤害」读数（下一步）。
- [x] **M13 HUD 预演读数 + 滚轮缩放**：
      **① 预演读数**：`interaction` 把「这一手会变成什么 · 哪一格 · 多远 · 预计多少伤害」
      整理成一行（`SKILL · cell (x,z) · dist d · dmg p`，"攻击"按距离显示成近战或火球），
      经 `PreviewReadout` 消息写进 HUD 左下角那条提示条——和"无法操作"共用位置，
      **被拒的输入优先**（暖色），读数用冷色，文案没变就不发消息。
      **② 滚轮缩放**：`input` 把 `MouseWheel` 归一成"格"（行 / 像素两种单位都支持）写
      `ZoomCamera`，`presentation::camera_zoom_system` 改 `CameraRig.zoom`（`offset` 按倍率缩放，
      上下限 0.45~1.9，每格 0.12）；中键拖拽平移保持不变。
      验收：`cargo test` **154 通过（152 单元 + 2 资产验收）/ 0 失败 / 0 跳过**
      （新增"滚轮拉远 / 拉近并夹在上下限"）、`cargo clippy --all-targets -- -D warnings`
      零警告、`cargo fmt --check` 通过；BRP 实机：`scroll_mouse` 滚 5 格后相机从
      `(34.00, 24.78, 31.79)` 退到 `(35.56, 26.60, 33.35)`（离注视点更远）。
- [x] **M14 打断 / 撤销 / 取消代价**（取代「确认」步骤；用户拍板的三条）：
      **① 键位重排**：`WASD` 让给技能热键（`HotkeyBinds`：`Q` 火球 / `W` 近战 /
      `E` 翻滚 / `R` 招架），移动只剩方向键；`1`~`4` 从"只选中"改成**选中 + 直接放**；
      `Space` 只表示暂停（跳跃挪到 `C`）；`F1` 帮助、`F2` 循环反应窗口、`F5` 重置。
      **② 反应窗口取代 `require_commit`**：`TimelineConfig { reaction: ReactionWindow }`
      三档——`Loose`（默认，只要有攻击瞄着玩家就停）/ `Strict`（只在玩家能反应时停）/
      `Off`；`ActionsCommitted` 连同"等确认"那一支一起删掉，`commit_bridge_system`
      现在只做"声明即升 `Pending`"。
      **③ 打断**：`timeline::interrupt_system`——本帧只要出现**玩家直接产生的意图**
      （`MoveCommand` / `MoveToCommand` / `JumpCommand` / `RollCommand` / `ParryCommand` /
      `UseSelectedSkill`）就把玩家那条**可取消的**未结算行动撤掉，后面的声明系统照旧接手。
      刻意**不监听** `FireCommand` / `MeleeCommand`：它们是 `UseSelectedSkill` 的下游、
      晚一帧才出现，监听会把"刚声明出来的火球"当成新意图撤掉（火球永远发不出去）。
      **④ 取消代价做成组件**：`ActionCost`（声明时花了多少 → 撤销原样退）、
      `CancelCost`（撤它要付多少；**没挂 = 免费**）、`Uncancellable`（跳跃：前摇里也撤不掉）；
      火球 `CancelCost(2)`、近战 `CancelCost(1)`、移动与翻滚免费——"赶路调整"不该收费，
      "大招打断"才该。用户否决了"取消有请求延迟"的版本：**不加固定硬直**。
      验收：`cargo test` **154 通过（152 单元 + 2 资产验收）/ 0 失败 / 0 跳过**
      （新增"撤销退款 + 取消代价"用例）、`cargo clippy --all-targets -- -D warnings` 零警告、
      `cargo fmt --check` 通过。
- [x] **M15 同帧阵亡崩溃 + 远射被冻在半路**（BRP 实机抓到的两个 bug）：
      **① 崩溃**：执行器收尾直接 `commands.entity(actor).insert(..)`，而行动者可能
      **在同一帧先被打死**（死亡系统销毁它）→ 命令应用阶段以 "Entity despawned" panic
      掉整个进程。改成全程 `Commands::get_entity` 守卫（`insert_on_actor` / `end_action_until`），
      回归用例 `ending_an_action_for_a_dead_actor_is_safe`。
      **② 按 3 没放出火球**：火球行动只忙"前摇 0.30 + 后摇 0.50"，而飞行时间 = 距离 / 8；
      玩家一恢复 `Ready`，`timeline_gate_system` 就冻结虚拟时间，球**停在半空**（精力却已扣）。
      修法：新增 `fireball::flight_time` 与 `SHOOT_HEIGHT`，执行器改用
      `end_action_until(.., executed_at + flight_time(..))`——和移动执行器同一套
      "忙到效果真的发生"。回归用例 `a_long_shot_keeps_the_shooter_busy_until_impact`
      （6 格外飞 1.5s）：回退修复时该用例会失败（"射手提前拿到决策权"）。
      验收：`cargo test` **154 通过（152 单元 + 2 资产验收）/ 0 失败 / 0 跳过**、
      `cargo clippy --all-targets -- -D warnings` 零警告、`cargo fmt --check` 通过；
      BRP 实机：敌人瞬移到 16 格外 → 按 3 → 1.4s 时玩家仍 `busy`、`TIMELINE · RUNNING`、
      敌 HP 未变 → 再等 1.5s → HP 38 → 26、战斗日志 +1（旧代码此刻已经 ready + 冻结）。
- [ ] **`Space` 手动暂停实际不起作用**（已知问题）：`timeline_gate_system` 先按"玩家就绪"
      暂停，`pause_toggle_system` 紧接着 unpause，下一帧门控又 pause——净效果是"只前进一帧"。
      修法：给 `Timeline` 加手动暂停标志，让门控尊重它（**待用户确认后动手**）。
- [ ] **箭矢还是旧的收尾方式**：`shoot_action_executor_system` 用 `end_action`，
      箭速 12、0.8s 只飞 9.6 米——超距的箭会像修好前的火球一样被冻在半空。
      改法与火球完全相同（**待确认**）。
- [ ] **收口**：`AGENTS.md` / `docs/status.md` 的按键 / 消息名 / 测试数已按本轮校正；
      剩下 `ecs-combat-components.md` 按 A 的新组件集改写。

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

- [x] 单位外观：玩家 / 敌人改用 2D 纸片 + 贴地阴影（`presentation/unit_sprite.rs`），
      素材 Kenney Tiny Dungeon（CC0）；换角色只需换 `assets/textures/units/*.png` 并改路径。
- [ ] 单位贴地与体素碰撞：`movement` 查 `world` 体素决定可否位移 + 跟随地形爬坡。
- [ ] 贪婪网格化 / 纹理图集 / AO（`voxel_render`）。
- [ ] 区块持久化（只存被改动的区块）+ 方块交互（`set_voxel`）。
- [ ] 接入 `textures/ground/grass.png`（当前无代码引用）。
- [x] 接入 CJK 字体：`assets/fonts/NotoSansSC-Regular.otf`（OFL-1.1），HUD 显式指定它，
      战斗日志的中文不再显示成豆腐块；覆盖由 `tests/assets.rs` 守着。
- [ ] 字体覆盖验收：`tests/assets.rs` 现在只查「字体文件在不在、是不是 sfnt」
      （`[dev-dependencies] skrifa` 已移除，逐字查 `cmap` 的验收随之删掉——字体已随仓库发版，
      覆盖情况改为运行时人工确认）。release 前重新评估要不要把 cmap 验收加回来。
- [ ] CJK 断行：Bevy 文本栈缺 `icu_segmenter` 的 CJK 分词模型，运行时会持续打印
      `ICU4X data error: No segmentation model for complex script`（正文仍正常渲染，
      只是断行退化）。B 用 `vendor/parley` 补丁解决，A 尚未处理。
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
| bevy_brp_extras | 0.22 | https://crates.io/crates/bevy_brp_extras | https://docs.rs/bevy_brp_extras/0.22 | 运行时调试协议扩展：截图 / 输入模拟 / 干净退出（仅 A，`rand 0.10.2` 同属 A；单位改 2D 纸片后不再需要 `bevy_ufbx`） |
| bevy_egui | 0.40.1 | https://crates.io/crates/bevy_egui | https://docs.rs/bevy_egui/0.40.1 | 调试面板运行时（仅 B） |
| bevy-inspector-egui | 0.37.0 | https://crates.io/crates/bevy-inspector-egui | https://docs.rs/bevy-inspector-egui/0.37.0 | egui 依赖（仅 B） |
| icu_segmenter | 2.3.0（features=auto） | https://crates.io/crates/icu_segmenter | https://docs.rs/icu_segmenter/2.3.0 | CJK 分词（仅 B，配 vendor/parley 补丁） |
| serde | 1.0.x | https://crates.io/crates/serde | https://docs.rs/serde/latest | 配置序列化（Phase 2.1，**未引入**） |
| ron | 0.12.x | https://crates.io/crates/ron | https://docs.rs/ron/latest | .ron 配置格式（Phase 2.1，**未引入**） |

引擎官方文档：https://bevy.org/learn/ · 迁移指南：https://bevy.org/learn/migration-guides/

## 验收标准（每次提交前）

**代码 A（仓库根）**

- [ ] `cargo test` 全绿（154 = 152 单元 + 2 资产验收，含 0 个 `#[ignore]`）
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
