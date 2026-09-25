# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概览

Project Timeless：基于 **Bevy 0.19.1** 的 roguelike 策略游戏原型。package 名 `app`，
源码在仓库根 `src/`。主线玩法是**无回合战斗时间线**——谁能决策由各自的 `DecisionSlot`
决定，每个动作自带前摇 + 后摇，世界在「暂停原因集合非空」时冻结。

`AGENTS.md` 是本仓库的风格与铁律全文（命令、命名、提交规范），**改动前先读它**；
本文件只补 AGENTS.md 没铺开的跨文件架构与踩坑点。文档入口是 `docs/index.md`。

## 常用命令

全部在**仓库根目录**执行：

```bash
cargo run                                   # 启动：体素地形 + 世界空间战斗
cargo test                                  # 全部测试（358：355 单元 + 3 资产验收）
cargo test <name>                           # 单个测试，例：cargo test fireball_flies_to_the_locked_cell_and_explodes
cargo test --lib                            # 只跑单元测试
cargo test --test assets                    # 只跑资产验收（改日志文案 / 加技能后必跑）
cargo clippy --all-targets -- -D warnings   # 必须零警告
cargo fmt --check                           # 必须通过
```

环境注意事项：

- 根 `Cargo.toml` **不是** workspace（无 `[workspace]`），**不要**加 `--workspace`。
- crates.io 直连不可用，依赖经**中科大（USTC）镜像**解析（`~/.cargo/config.toml`）。
  **不要用 `cargo add`**（已知兼容性问题）；依赖手动写进 `Cargo.toml`，
  并在 `TODO.md` 的版本索引表登记。
- **依赖版本只在改过 `Cargo.toml` 之后才刷新本地索引缓存**：解析失败可能只是缓存旧
  （踩过：`file_watcher` 曾被误判为"镜像没有"）。
- 开发期热重载：`cargo run --features hot-reload`（生产构建不带这个 feature）。
- 提交前三条必须全过（test / clippy / fmt）。**禁止用 `#[ignore]` 隐藏失败**——
  要么修好，要么在 `TODO.md` 写明根因与下一步。
- `main.rs` 启用了 `bevy_remote` + `bevy_brp_extras`：运行时可用 BRP 查询 / 模拟输入 / 截图，
  但**组件必须 `register_type` 才会出现在反射表里**（HUD 标记组件因此逐个注册）。

## 架构：需要读多个文件才能拼出来的部分

### 一个领域 = 一个目录 = 一个 `Plugin`

顶层域：`world`（体素数据，零渲染依赖）· `voxel_render`（网格化 / 材质 / 明暗）·
`movement`（`Cell` 决策 + `Velocity` 位移 + 移动 / 跳跃 / 翻滚）· `combat`（战斗全部子域）·
`skills`（技能**静态定义**：`AbilityId` / `AbilityDef` / `can_cast`）·
`equipment`（装备：槽位 / 物品 / 「基础值 + 加成」）· `timeline`（无回合调度）·
`ai`（敌人战术）· `input`（键盘 / 鼠标 → 消息）· `interaction`（鼠标拾取 / 点击翻译 / 高亮与预演画面）·
`presentation`（相机 / 纸片 / HUD / 日志，只读）· `spawn`（组装车间）。

两处**域内分层**值得照抄：`interaction/` 分 `pointer.rs`（翻译，不产生实体）与
`visual.rs`（画面，只读状态）；`presentation/hud/timeline/` 分 `model.rs`（纯函数 +
快照，可脱离 App 单测）/ `scene.rs`（UI 夹具）/ `system.rs`（取数 → 比对 → 写 UI）。
新增 UI 一律照这个三层走，别把几何、夹具与逐帧写入塞回一个文件。

- 「玩家」「敌人」**不是模块**，是组件的组合体：零件归各领域（`Health` → combat、
  `Velocity` → movement、`EnemyBrain` → ai、`ChunkLoader` → world），组装归 `spawn/`。
  **没有任何域依赖 `spawn` 的组装逻辑**（唯一例外：`input` 写 `spawn::ResetBattle`）。
- `combat` 是**一个** `CombatPlugin` 装着 8 个子域（`attributes` / `health` / `targeting` /
  `lifecycle` / `formula` / `skills` / `defense` / `reaction`）；「每个子域自己的 `plugin.rs`、
  父域只编排」仍是目标设计（见 `docs/domain.md` 与 `TODO.md`）。
- 移动 / 跳跃 / 翻滚**也是技能**，与火球、横扫走同一条路，不做特殊处理。

### 执行顺序是语义的一部分，不是性能选择

跨域顺序只在 `src/lib.rs::configure_pipeline` 里声明一次（测试复用同一入口，
跑的就是真实流水线顺序）：

```text
Startup:  PreloadSet ─▶ AssemblySet
Update:   SpawnSet ─▶ InputSet ─▶ InteractionSet ─▶ TimelineSet ─▶ AiSet
          ─▶ MovementSet ─▶ EquipmentSet ─▶ CombatSet ─▶ VoxelRenderSet ─▶ PresentationSet ─▶ ClockSet
WorldSet ──────────────────────▶（必须早于 VoxelRenderSet）
```

- `InputSet` 在 `AiSet` 前：玩家这一帧的表态先落地。
- `AiSet` 在 `CombatSet` 前：**敌人先决策**，威胁扫描（`combat::reaction`）才扫得到它刚生成的行动。
- `EquipmentSet` 在 `CombatSet` 前：装备加成要在命中公式读它之前算好（有效护甲 / 格挡率）。
- `ClockSet` 排在帧末：这一帧所有系统看到同一个冻结状态，唯一的 `Time<Virtual>` 写入点在这里。

**一处改动会牵动整条链**：改声明顺序或往管道里加系统集，必须同时想清
「这一帧谁先看到什么」和「暂停在何时生效」，不要只看单个领域的测试是否绿。

### 无回合模型的三根支柱

| 概念 | 管什么 | 住在哪 |
| :--- | :--- | :--- |
| 决策槽 `DecisionSlot` | 这个单位此刻能不能决策（`Idle { intent }` / `Executing { until }`） | 单位身上 |
| 行动实体 | 这一手什么时候落地（`ScheduledAction.execute_at` + 载荷 + `ActionOf`） | 独立实体 |
| 时钟 | 世界现在停不停 | `Time<Virtual>`，只由 `clock::process_pause_requests` 写 |

- 「行动实体化」：行动 = 独立实体（载荷组件 + `ScheduledAction` + 可选的 `Uncancellable`）。
  归属是**自定义关系** `ActionOf` / `Actions`（`linked_spawn`），**不是 `ChildOf`**——行动没有
  `Transform`。人死了名下行动跟着销毁，孤儿行动构造不出来。
- 调度器**不感知载荷**：新增动作 = 新增载荷组件 + 新增执行器，调度器不动。
- **执行器自己收尾**，没有集中式收尾函数：`if !schedule.due(now) { continue; }` → 落地效果 →
  销毁行动实体 → 给行动者写 `DecisionSlot::recovering(...)`。
- **「效果延迟发生」的动作（移动 / 火球 / 箭矢）必须把 `effect_delay` 给到效果真正发生的那一刻**
  （走到格中心 / 飞到落点）。给少了，行动者一空闲虚拟时间就冻住，效果停在半路、伤害不结算。
  这条是整机级 bug，单域测试看不出来（见 `src/lib.rs` 的
  `a_long_shot_keeps_the_shooter_busy_until_impact`）。
- **冻结设施住在独立的 `clock` 域**（不属于任何领域）：各域这一帧还想停表就写
  `PauseRequest::Pause(reason)`（`clock::AWAITING` / `clock::THREAT`）——**每帧断言**，
  下一帧不再写原因就自然消失，不需要撤销；玩家按键写一次性的 `Toggle`
  （冻着就放开、没冻就置 `ManualPause`）。
  判据是 `PauseReasons 非空 || ManualPause`，只有帧末 `ClockSet` 的
  `process_pause_requests` 能写 `Time<Virtual>`；**任何地方都不许手写 `if paused` 门控**。

### 坐标与分层

- **决策按格、结算按真实距离**：`Cell`（边长 `CELL_SIZE`）只用于决策与同格判定；
  命中 / 射程 / 爆炸半径一律用世界距离。位置只有一份真相（`Transform`），`Cell` 只在单位停下时更新。
- **领域层零 Bevy**：`combat/formula/domain.rs` 绝不引入 Bevy，可脱离 App 单测；应用层不写伤害公式。
- **`world` 是纯数据域**：不引用 `Mesh3d` / `StandardMaterial` / `Assets<…>`，可用 `MinimalPlugins` 单测；
  网格化与材质一律在 `voxel_render`，两域只经区块消息通信。
- **功能不是领域**：只把已有系统拼一次的胶水留在调用方（如 `spawn/restart.rs`）。
- **形状 = 一个形状一个组件**（`HitRadius` / `MeleeShape`），没有中心化 `Shape` 枚举，
  也**没有 `utils` 域**——判定系统紧贴各自的形状（`combat/targeting/`）。

### BSN（`bsn!` + `spawn_scene`）

局内所有**一次性的实体组装**都用 `bsn!`；长元组 `spawn((...))` 只该出现在测试里。

- 裸写组件名即可（`Name("X")` / `HoverHighlight` / `Transform { .. }`），
  组件要 `Clone + Default` 才能这么写（`bevy_ecs::template` 的 blanket impl）。
- **非 Default 的初值用 `template_value(x)`**（`DecisionSlot::Idle { intent: None }`、`Health::new(50)`、`ActionOf(actor)`）。
- 现场造资产用 `asset_value(expr)`，里面是**普通 Rust 表达式**，可以写 `..default()`。
- 字段值不是字面量时包 `{expr}`；**不要写 `..default()` 做结构体剩余字段**
  （那是 `asset_value` 里才有的写法）。
- `Children [a, b]` 的元素必须是 `Scene` 而不是 `Bundle`——现有 UI 工厂返回 `impl Bundle`，
  所以 HUD 根挂子节点走 `add_children`（见 `presentation/hud/layout.rs`）。
- **`Commands::spawn_scene` 需要 `AssetServer` + `ScenePlugin`**：`world` 域（要能用裸
  `MinimalPlugins` 单测）和 `voxel_render` 的网格化（手里是 `Handle`，不建材质）都因此
  继续用 `spawn`。给 `world` 加 BSN 会破坏它的零渲染依赖铁律。
- 用 `bsn!` 的场景工厂**必须有测试钉住组件真的挂上了**：漏写一行不报编译错，
  只表现为「高亮永远显示 / 永远不显示」（`interaction/visual.rs` 的
  `the_scene_factories_attach_their_markers` 是范例）。

### 通信

- 跨模块一律 `MessageWriter` / `MessageReader`；只有「立即生效 + 针对具体实体」才用
  `EntityEvent` + Observer，两者不可混用（Bevy 0.19 已移除 `EventReader` / `EventWriter`）。
- **消息定义在消费它的领域**（消费方在插件 `build` 里 `add_message::<T>()`），注释写清「谁写、谁消费」。
  完整跨域消息清单见 `docs/domain.md` 第二节。
- **`input` 只翻译、不执行**：按键本身永远住在 `input/`（空格等待、`P` 暂停、
  `B`/`V` 方块、`T` 穿脱装备、`F5` 重置、`Q/W/E/R` 技能），
  其它域只收到「暂停一下」「重置一下」这类意图，不认识 `KeyCode`。
- **组件可以跨域读，行为不许跨域调**：数据契约（组件类型、纯类型 / 常量、纯函数）可直接引用；
  别人的系统、别人的 `Resource`、一次操作的请求 / 结果必须走消息或事件。

## 测试基建

- 单元测试写在源码旁的 `#[cfg(test)] mod tests`；`src/lib.rs` 的 `mod tests` 放**整机用例**
  （真实流水线顺序），资产验收在 `tests/assets.rs`。
- `crate::test_support::headless_app()`（`src/lib.rs`）装齐「除渲染外」的整机 App；
  `src/lib.rs` 内的 `test_app()` 是更小的夹具（无 Presentation / Spawn）。
- 时间用 `TimeUpdateStrategy::ManualDuration(Duration::from_millis(100))` 手动步进，
  因此测试能精确走完「声明 → 前摇到点 → 执行器落地 → 后摇恢复」的整条时间线。
- 加按键要投递真实的 `KeyboardInput` 消息（见 `press` 辅助函数）——直接改 `ButtonInput`
  会被 `bevy::input::InputPlugin` 每帧重建按钮状态时清掉。
- 轻量夹具里缺某个领域时，它消费的消息要手动 `add_message::<T>()` 补上（`test_app` 里有一串例子）。
- **加技能 = 加一条 `AbilityDef` + 一张图标 PNG**：`tests/assets.rs` 会要求
  `icon_path(kind)` 指到的文件存在、是带 alpha 的方图。改日志文案后也必须跑它。

## 文档地图（改动后同步）

| 文档 | 回答什么 |
| :--- | :--- |
| `TODO.md` | **唯一的进度真相**：里程碑勾选、backlog、依赖版本索引。勾选必须有验收证据 |
| `docs/index.md` | 文档唯一入口与维护规则 |
| `docs/domain.md` | 域地图、跨域契约、执行顺序、**十二条铁律** |
| `docs/relations.md` | 物理附着（`ChildOf`）vs 逻辑关系（自定义关系） |
| `docs/timeline.md` / `docs/skills.md` / `docs/combat.md` | 时间线 / 技能 / 战斗专题（🚧 = 目标设计，代码未落地） |
| `docs/equipment.md` | 装备：槽位 / 物品 / 穿脱 / **属性叠加（基础 + 加成）**（已落地）· `docs/insight.md` 是洞察力设计稿 |
| `docs/bevy-019.md` | **写任何 Bevy 代码之前**必读：0.19 API 与迁移清单 |

## 风格要点

- `rustfmt` 默认配置（4 空格，edition 2024）；Rust 标准命名。
- **注释与文档注释用中文，标识符与提交信息用英文**；提交用 Conventional Commits
  （`feat:` / `fix:` / `docs:` / `refactor:` / `test:`），每次提交只含一个逻辑变更。
- 实体构建优先用 **BSN**（`bsn!` + `spawn_scene`），组件派生 `Default + Clone`；
  组合实体用场景语法，不用长元组 `spawn((...))`。
- **HUD 文案用英文，战斗日志正文用中文**，因此 HUD 显式指定 `hud::HUD_FONT`
  （`assets/fonts/NotoSansSC-Regular.otf`，Bevy 默认字体不含 CJK）。
- 领域内部按概念分文件，一个文件回答一个问题；**系统跟着它操作的数据走**，
  不要为凑一个 `systems.rs` 把不相干的系统堆在一起；`mod.rs` 只做 `pub mod` + `pub use` 门面。

细节与完整铁律见 `AGENTS.md` 与 `docs/domain.md` 第五节。