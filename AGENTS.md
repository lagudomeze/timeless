# 仓库指南（Repository Guidelines）

Project Timeless 是基于 Bevy 0.19 的 roguelike 策略游戏。主线玩法是**无回合**的战斗
时间线：谁能决策由各自的 `DecisionSlot` 决定，每个动作自带前摇 + 后摇，
世界在**暂停原因集合非空**时冻结（`Time<Virtual>`）：等玩家输入、手动暂停、威胁逼近。

代码在仓库根目录 `src/`（package `app`）。文档入口见 [`docs/index.md`](docs/index.md)，
进度与 backlog 只有一处：[`TODO.md`](TODO.md)。

## 项目结构

```
根目录
├── AGENTS.md                 # 本指南：命令、风格、铁律
├── TODO.md                   # 唯一的进度真相：里程碑勾选 + backlog + 依赖索引
├── Cargo.toml / Cargo.lock   # package `app`（**不是** workspace）
├── src/                      # 游戏代码，一个领域 = 一个目录 = 一个 Plugin
├── assets/                   # 素材（LICENSES.md 可追溯）
├── tests/assets.rs           # 资产验收：字体是合法 sfnt、精灵与图标是带 alpha 的方图
├── skills/                   # Codex 技能（与 src/skills/ 无关）：bevy-019-docs / bevy-assets
└── docs/                     # 架构 / 时间线 / 组件对照 / 设计 / 素材 / Bevy 速查
```

领域：`world`（体素数据，零渲染依赖）· `voxel_render`（网格化 / 材质 / 明暗）·
`movement` · `combat` · `skills`（技能**静态定义**）· `timeline` · `ai` · `input` ·
`interaction` · `presentation` · `spawn`（组装车间）。纯几何不单独建域——形状是
**一个形状一个组件**（`HitRadius` / `MeleeShape`），判定紧贴各自的系统。
完整说明见 [`docs/domain.md`](docs/domain.md)。

## 构建、测试与开发命令

全部在**仓库根目录**执行：

```bash
cargo run                                   # 启动：体素地形 + 世界空间战斗
cargo test                                  # 251 通过（248 单元 + 3 资产验收）/ 0 跳过
cargo clippy --all-targets -- -D warnings   # 必须零警告
cargo fmt --check                           # 格式校验
```

根 `Cargo.toml` **不是** workspace（无 `[workspace]`），请不要用 `--workspace`。

环境注意事项：本机 crates.io 直连不可用，依赖经**中科大（USTC）镜像**解析
（配置在 `~/.cargo/config.toml`，`replace-with = 'ustc'`）。**不要使用 `cargo add`**
（已知兼容性问题）；依赖须手动写入 `Cargo.toml`，并在 `TODO.md` 的版本索引表登记。

依赖版本**只在改完 `Cargo.toml` 后**才更新本地索引缓存——不改而只跑 `cargo build` / `check`
时，解析失败可能只是缓存旧，不代表镜像没有那个版本（踩过：见 `TODO.md`「开发热重载」）。

## 编码风格与命名规范

- 遵循 `rustfmt` 默认配置（4 空格缩进，edition 2024）。
- Rust 标准命名：函数 / 变量 / 测试用 `snake_case`，类型与枚举变体用 `CamelCase`。
- 注释与文档注释（模块级 `//!`、条目级 `///`）用中文；标识符与提交信息用英文。
- **严格分层**：领域层 `src/combat/formula/domain.rs` 绝不引入 Bevy，
  可脱离渲染单测；应用层不包含伤害公式；模块间仅通过 Bevy `Message` 通信。
- **高内聚低耦合**：每个领域文件只装自己的组件 / 消息 / 系统。移动领域只含
  格子坐标、位移行动与投射物飞行；火球 / 爆炸等战斗内容归 `combat`，
  通过 `ProjectileArrived` 衔接。
- **动作实体化**：行动 = 独立实体（载荷组件 + `ScheduledAction` + 可选的
  `Uncancellable`），场景工厂把 `ActionOf({actor})` 写进模板（即行动实体生下来就
  记着自己归谁）；归属是**自定义关系**（`ActionOf` / `Actions`，`linked_spawn`），
  **不是 `ChildOf`**——行动没有 `Transform`，物理附着这个词不该被挪用
  （见 [`docs/relations.md`](docs/relations.md)）。调度器不感知载荷，新增动作
  只需新增载荷与执行器。**行动者的阶段写在决策槽里**（`DecisionSlot::{Idle { intent },
  Executing { until }}`），时间戳只回答「到点没有」
  （`now < execute_at` 前摇 / `now > execute_at` 该执行），
  不再有 `Declared` / `Pending` / `Committed` 这类标记。暂停用 `Time<Virtual>`，
  不手写阶段门控。
- **实体构建优先用 BSN**（`bsn!` + `spawn_scene`）：组件派生 `Default + Clone`
  （含 `Entity` 字段的派生 `FromTemplate`），多个组件组合成实体用场景语法，
  不用长元组 `spawn((...))`；字段值若非字面量，一律包 `{expr}`。

## 输入与通信规范

- **UI 输入只翻译、不执行**：键盘 / 鼠标不得直接修改游戏状态，只把操作翻译成
  Bevy `Message`，再由对应领域的单一职责系统消费落地。当前消息清单见
  [`docs/domain.md`](docs/domain.md) 第二节。
- **消息定义与消费它的系统同属一个领域**：如 `MoveCommand` 与 `declare_move_system`
  在 `movement/`、`FireCommand` 与 `declare_fireball_system` 在 `combat/attack/`、
  `PauseRequest` / `PlayerTakeover` / `UndoCommand` 与
  `compute_player_awaiting_system` / `undo_system` /
  `process_pause_requests` 在 `timeline/`、
  `PointerCommand` 与 `pointer_command_system` 在 `interaction/`。
  其他领域需要该操作时只写消息，不重复实现。
  **按键本身永远住在 `input/`**：空格是暂停、`F5` 是重置、`Q/W/E/R` 是技能，
  各领域只收到"暂停一下""重置一下"这类意图，不认识 `KeyCode`。
- 跨模块交互一律走 `MessageWriter` / `MessageReader`；只有需要立即生效、
  针对具体实体时才用 Event + Observer，两者不可混用。
- 新增消息在**消费方领域**的插件 `build` 里用 `add_message::<T>()` 注册
  （输入域只写消息，不注册别人的消息），并在消息上注明「谁写、谁消费」。

## 模块约定

- `main.rs` 只组装插件（引擎插件 + `GamePlugin`）；跨领域的执行顺序只在
  `lib.rs::configure_pipeline` 里用 `SystemSet` 声明一次，领域内部顺序由各自
  `plugin.rs` 维护（测试复用同一入口，保证跑的是真实流水线顺序）。
- 领域内部按**概念**分文件，一个文件回答一个问题；`components.rs` / `events.rs`（消息）/
  `resources.rs` / `systems.rs` 是默认分法，但当某个概念的数据与它的系统天然成对时，
  可以合成一个文件（如 `timeline/decision.rs` 同时装 `DecisionSlot` 与 `undo_system`）。
  **系统跟着它操作的数据走**，不要为了凑一个 `systems.rs` 把不相干的系统堆在一起。
  `mod.rs` 只做 `pub mod` + `pub use` 门面。
- **组合域只编排**：`combat` / `world` / `voxel_render` 这类装着子域的父域，父域
  `Plugin` 只负责**子域之间的顺序**，不自己注册系统。现状是 `combat` 7 个子域
  仍由一个 `CombatPlugin` 直接接线（子域 `plugin.rs` 是目标，见 `docs/domain.md`）。
- **角色实体不是模块**：零件归各领域（`Health` → combat、`Velocity` → movement、
  `EnemyBrain` → ai、`ChunkLoader` → world），组装归 `spawn/`（`unit_scene` 给共用
  零件，`player.rs` / `enemy.rs` 追加驱动源）；**没有任何领域依赖 `spawn` 的组装逻辑**
  （唯一的例外是输入域写 `spawn::ResetBattle` 这一条消息，它由 `spawn` 消费）。
- **执行器自己收尾**：`if !schedule.due(now) { continue; }` → 落地效果 → 销毁行动实体 →
  给行动者写 `DecisionSlot::recovering(timing, now, effect_delay)`。没有集中式收尾函数；
  「效果延迟发生」的动作（移动 / 火球 / 箭矢）必须把 `effect_delay` 给到效果真的发生
  （走到格中心 / 飞到落点），否则玩家一空闲世界就冻住、效果停在半路。
- **暂停是两种时序**：各领域这一帧还想停表就写一条
  `PauseRequest::Pause(reason)`（`"awaiting"` / `"threat"`）；下一帧不再断言，
  原因自然消失，不需要谁去撤销。玩家的空格是**翻转**：`PauseRequest::Toggle("manual")`
  冻着就清空原因（世界立刻动）、没冻就停住并闩进 `timeline::LatchedReasons`。
  **两种时序别混**：断言每帧重来，按键是一次性事件；共用一条消息就得从原因集合猜
  "上一帧谁断言过"，而集合里混着别人的原因，猜不准（踩过：威胁冻着时按空格被误判成
  "暂停"而不是"继续"）。
  **翻转必须先于各领域的断言落地**（`apply_pause_toggles_system` 排在 `TimelineSet`
  最前），否则同一帧里"威胁还在断言 `Pause(THREAT)`"会把玩家刚清掉的原因加回来；
  各领域的断言统一在帧末 `process_pause_requests` 收进集合，唯一的 `Time<Virtual>`
  写入点是 `ClockSet` 的 `apply_clock`（Bevy 每帧把虚拟时间拷进通用 `Time`，
  位移 / 投射物 / 后摇自动停表）；禁止在其它地方手写 `if paused` 阶段门控。
- **威胁窗口是边沿开的、持续按住的**（`combat::reaction`）：窗口状态住在
  `ThreatWindow`（`open` / `dismissed`），威胁检测**每帧**在窗口开着时继续断言
  `Pause(THREAT)`（世界冻着时来源与移动都停在半路，窗口该一直开着）；
  玩家按空格 → 原因集合被清空 → 检测系统读到"没有 `THREAT`"，关窗并记 `dismissed`，
  **同一次威胁不再重开**：那一击照常落地（**忍受伤害也是一种决策**）。
  `Threatened` 只是打在威胁源上的可读标记（BRP 锚点），不参与判定。
- **坐标：决策按格、结算按真实距离**。格（`movement::Cell`，边长 `CELL_SIZE`）只用于
  决策与同格判定；命中 / 射程 / 爆炸半径一律用世界距离。位置只有一份真相
  （Bevy `Transform`），`Cell` 只在单位停下时更新。
- **表现层只读**：HUD 与相机平移在 `presentation/`，只读游戏状态。
  HUD 按屏幕位置拆成五块（顶部时间轴 / 左下玩家面板 / 右下敌人面板 / 底部技能栏 /
  右下偏上日志 + 居中帮助），分辨率适配靠 `fit_ui_scale_system`。
  **HUD 文案用英文，战斗日志正文用中文**，因此 HUD 显式指定 `hud::HUD_FONT`
  （`assets/fonts/NotoSansSC-Regular.otf`）而不是 Bevy 默认字体。
  改日志文案后**必须**跑 `tests/assets.rs`。
- **单位外观是 2D 纸片 + 贴地阴影**：纸片绕 Y 轴对准相机，高度靠正下方地表上的
  黑色阴影表示。精灵与阴影是单位实体的子节点，因此单位根节点必须保持
  「脚底 + 无旋转 + 无缩放」。
- **`world` 是纯数据域**：不引用渲染类型（`Mesh3d` / `StandardMaterial` /
  `Assets<…>`），可用 `MinimalPlugins` 单测；网格化 / 材质一律放 `voxel_render`，
  两域只经区块消息通信。
- **功能不是领域**：只把已有系统拼一次的胶水留在调用方（如 `spawn/restart.rs`），
  有数据模型 / 规则才进领域。

## 测试规范

- 单元测试写在源码旁的 `#[cfg(test)] mod tests` 中；`src/lib.rs` 的 `mod tests`
  放**整机用例**（真实流水线顺序 + `headless_app`），其余散在各领域文件里的
  纯逻辑用例；资产验收在 `tests/assets.rs`。
- 测试名用描述性的 `snake_case`，例如 `pressing_walks_exactly_one_cell_and_stops_at_its_center`。
- 用 `assert_eq!`；断言意图不直观时附简短说明。
- **不要用 `#[ignore]` 隐藏失败**：要么修好，要么在 `TODO.md` 写明根因与下一步。
- 提交前必须通过：`cargo test` 全绿、
  `cargo clippy --all-targets -- -D warnings` 零警告、`cargo fmt --check` 通过。

## 提交与 Pull Request 规范

采用 Conventional Commits：`feat:` / `fix:` / `docs:` / `refactor:` / `test:` 前缀
（如 `feat: add fireball skill`），每次提交只含一个逻辑变更。
Pull Request 需说明改动内容与动机、关联 `TODO.md` 中的条目，并记录手工验证结果
（如控制台「谁先命中」输出）。
