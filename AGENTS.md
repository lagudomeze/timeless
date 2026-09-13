# 仓库指南（Repository Guidelines）

Project Timeless 是基于 Bevy 的 roguelike 策略游戏，主线玩法是**无回合**的战斗时间线：
谁能决策由各自的 `Ready` 决定，每个动作自带前摇 + 后摇，只在玩家等输入时冻结世界。
Cargo workspace 位于 `timeless/`（代码 B）；仓库根目录的 `src/` 是主线原型（代码 A），
另放本指南、`TODO.md` 与 `docs/` 文档。

## 项目结构与模块组织

```
根目录：
├── AGENTS.md                  # 本指南
├── TODO.md                    # 里程碑、依赖版本索引、环境注意事项
├── docs/                      # 设计文档（design / bevy / art）
├── src/                       # app 原型：Bevy 0.19 世界空间纵切（见 docs/design/app-modules.md）
└── timeless/                  # cargo workspace（子项目）
timeless/
├── crates/timeless-domain/   # 领域层：纯 Rust，零 Bevy 依赖（战斗裁决、网格）
├── crates/timeless-app/      # 应用层：Bevy 组件、系统、渲染、HUD
└── vendor/parley/            # 本地补丁：CJK 分词（见 README.patch.md）
```

**代码 B（`timeless/`，已冻结）**：领域类型（`AttackStats`）定义在 `timeless-domain`；
网格坐标用 `IVec2`（`Position` / `Roll` / `Destination` 包装它，网格数学以扩展 trait
落在应用层）。**本检出跑不起来**（`timeless-app/assets/` 目录不存在、
`vendor/parley` 补丁不存在），因此不要把 B 的命令当作验收门槛。

**代码 A（`src/`，主线）**：决策层坐标是 `movement::Cell { x, z }`（不是 `IVec2`），
结算用真实世界距离；领域层纯逻辑在 `combat/formula/domain.rs`（`AttackStats` 是
纯函数入参，不是 ECS 组件）。按「一个领域 = 一个目录 = 一个 Plugin」组织，详见
[docs/design/app-modules.md](docs/design/app-modules.md)。

## 构建、测试与开发命令

以下命令均在 `timeless/` 目录下执行（**B 已冻结，仅供参考**）：

- `cargo run -p timeless-app` — 启动游戏。
- `cargo test --workspace` — 运行全部单元测试（领域层 7 + 应用层 2 = 9 个）。
- `cargo clippy --workspace` — 静态检查，必须零警告。
- `cargo fmt` / `cargo fmt --check` — 格式化代码 / 校验格式。

根目录 `src/` 原型（package `app`）在**仓库根目录**执行：

- `cargo run` — 启动原型（体素地形 + 世界空间战斗）。
- `cargo test` — 原型测试：`src/` 下 152 个单元测试 + `tests/assets.rs` 2 个资产验收用例（字体可解析、单位精灵与技能图标是带 alpha 的方图）。
- `cargo clippy --all-targets -- -D warnings` / `cargo fmt --check` — 同 timeless 的验收标准。

环境注意事项：本机 crates.io 直连不可用，依赖经清华镜像解析。不要使用 `cargo add`（已知兼容性问题）；依赖须手动写入 `Cargo.toml`，并在代码中引入前更新根目录 `TODO.md` 的版本索引表。

## 编码风格与命名规范

- 遵循 `rustfmt` 默认配置（4 空格缩进，edition 2024）。
- Rust 标准命名：函数、变量、测试用 `snake_case`；类型与枚举变体用 `CamelCase`。
- 注释与文档注释（模块级 `//!`、条目级 `///`）使用中文；标识符与提交信息使用英文。
- 严格分层：领域层（A 的 `combat/formula/domain.rs` / B 的 `timeless-domain`）
  绝不引入 Bevy，可脱离渲染单测；应用层不包含伤害公式；模块间仅通过 Bevy `Message` 通信。
- 高内聚低耦合：每个领域文件只装自己的组件 / 消息 / 系统，组件只表达自己的职责，
  不给无关系统夹带状态。例如移动领域（`movement.rs`）只含网格坐标、位移行动与投射物飞行；
  火球 / 爆炸等战斗内容归 `combat.rs`，通过 `ProjectileArrived` 消息衔接。
- 动作实体化：行动 = 独立实体（载荷组件 + `ScheduledAction` + `Declared` / `Pending` /
  `Committed` 状态标记），调度器不感知载荷，新增动作只需新增载荷与执行器，不改调度器；
  复杂交互走两阶段结算（阶段 1 计算 `CombatResult`，阶段 2 统一应用）；暂停用
  `Time<Virtual>`，不要手写阶段门控。
- 实体构建优先用 BSN（`bsn!` + `spawn_scene`）：组件派生 `Default + Clone`
  （含 `Entity` 字段的派生 `FromTemplate`），多个组件组合成实体用场景语法，
  不用长元组 `spawn((...))`；字段值若非字面量，一律包 `{expr}`。
  含 `Handle` 字段或复杂子实体树的生成（投射物、战斗单位）暂可保持 `spawn` 元组，
  待 BSN 资产模板验证后迁移。

## 输入与通信规范

- **UI 输入只翻译、不执行**：键盘 / 鼠标等输入源不得直接修改游戏状态，只把按键
  翻译成 Bevy `Message`，再由对应领域的单一职责系统消费并落地。禁止在输入系统里
  同时做「翻译 + 决策 + 改状态」。A 原型现有的输入消息：`MoveCommand` / `JumpCommand` /
  `FireCommand` / `MeleeCommand` / `RollCommand` / `ParryCommand` / `SelectSkill` /
  `CycleSkill` / `UseSelectedSkill` / `PointerCommand` / `TogglePause` /
  `CycleReactionWindow` / `ToggleHelp` / `PanCamera` / `ZoomCamera`。
- **消息定义与消费它的系统同属一个领域**：如 `MoveCommand` 与 `declare_move_system`
  在 `movement/`、`FireCommand` 与 `declare_fireball_system` 在 `combat/skills/`、
  `RollCommand` 与 `declare_roll_system` 在 `combat/defense/`、
  `TogglePause` / `UndoCommand` 与 `pause_toggle_system` / `undo_system` 在 `timeline/`、
  `PointerCommand` 与 `pointer_command_system` 在 `interaction/`。
  其他领域需要发起该操作时只写消息，不重复实现；不要在生产者文件里定义消费方领域的消息。
- 跨模块交互一律走 `MessageWriter` / `MessageReader`；需要立即生效、针对具体
  实体时才用 Event + Observer，两者不可混用。
- 新增消息在**消费方领域的插件** `build` 里用 `add_message::<T>()` 注册
  （输入域只写消息，不注册别人的消息），并在消息上注明「谁写、谁消费」。

## 根目录 app 原型的模块约定

- `main.rs` 只组装插件（引擎插件 + `GamePlugin`）；跨领域的执行顺序只在
  `GamePlugin::configure_pipeline` 里用 `SystemSet` 声明一次，领域内部顺序由各自
  `plugin.rs` 维护（测试复用同一入口，保证跑的是真实流水线顺序）。
- 领域内部按职责分文件：`components.rs` / `events.rs`（消息）/ `systems.rs` /
  `resources.rs`；`mod.rs` 只做 `pub mod` + `pub use` 门面。
- **角色实体不是模块**：零件归各领域（`Health` → combat、`Velocity` → movement、
  `EnemyBrain` → ai、`ChunkLoader` → world），组装归 `spawn/`（`unit_scene` 给共用
  零件，`player.rs` / `enemy.rs` 追加驱动源）；**没有任何领域依赖 `spawn`**。
- **行动实体化 + 时间线（无回合）**：行动是独立实体（载荷组件 + `ScheduledAction` +
  `Declared` / `Pending` / `Committed`），由 `timeline/` 调度；调度器不感知载荷，
  载荷与执行器归各自领域（`movement/actions.rs`、`combat/skills/*.rs`）。
  **没有「轮」也没有规划阶段**：谁能决策由单位自己的 `Ready` 决定，
  每个动作自带固定前摇 + 后摇（`timeline::timing`），同一单位忙的时候不接受新声明。
  执行器收尾**必须**走 `timeline::end_action`（摘 `Committed` + 销毁行动实体 + 挂后摇），
  否则执行器会每帧重复触发同一个动作。
- **暂停等输入**：唯一的暂停点是 `timeline_gate_system`——玩家就绪且未在空中时冻结
  `Time<Virtual>`（Bevy 每帧把虚拟时间拷进通用 `Time`，位移 / 投射物 / 后摇自动停表）；
  禁止在其它地方手写 `if paused` 阶段门控。
  键位：**方向键**走一格 · **左键**点地板 = 走到那一格、点单位 = 用当前选中的技能打
  那一格 · **右键**撤销未结算的行动 · `1`~`4` 直接放技能 · `Tab` 循环 ·
  `Q`/`W`/`E`/`R` 技能热键（`HotkeyBinds`） · `C` 跳跃 · `Space` 暂停 ·
  **`F1` 开合帮助面板** · `F2` 循环反应窗口（`Loose`/`Strict`/`Off`） · `F5` 重置 ·
  按住鼠标中键拖拽平移相机 · 滚轮缩放。
  **没有"确认"步骤**：声明即生效，反悔靠打断 / 撤销（`CancelCost` 定代价，
  `Uncancellable` 的行动撤不掉）。
  移动方向按**屏幕**算（上 = 远离相机），由 `input` 的 `GroundBasis` 按相机朝向
  换算到世界 XZ 平面，再经 `movement::step_from_axis` **吸附成一格的正交步**；
  平面轴约定见 `movement::ground_direction`。
- **坐标：决策按格、结算按真实距离**：格（`movement::Cell`，边长 `CELL_SIZE`）只用于
  决策与同格判定；命中 / 射程 / 爆炸半径一律用世界距离。单位只在停下时更新 `Cell`
  （`move_entities_system` 吸附到目标格中心），不每帧从 `Transform` 反推。
- **表现层只读**：HUD 与相机平移都在 `presentation/`，只读游戏状态。
  HUD 按屏幕位置拆成五块（`presentation/hud/`）：顶部时间轴（行动色块，蓝 = 玩家 /
  红 = 敌人，未提交的草案半透明）、左下玩家面板 / 右下敌人面板（头像 + HP / EN 条 +
  状态行 + 当前行动）、底部居中技能栏（图标 + 消耗角标 + 悬停 tooltip）、右下偏上
  可折叠战斗日志、居中帮助面板（`F1`）。**常驻按键提示不放在屏幕角落**——它们只在
  帮助面板里。分辨率适配靠 `fit_ui_scale_system`（窗口高 / `BASE_HEIGHT` → `UiScale`），
  面板尺寸用像素、锚点用百分比。
  **文本语言与字体**：HUD 自己的文案用英文；**战斗日志正文是中文**
  （`presentation/log.rs`），因此 HUD 显式指定 `hud::HUD_FONT`
  （`assets/fonts/NotoSansSC-Regular.otf`，OFL-1.1）而不是 Bevy 默认字体
  （默认字体不含 CJK，会显示成豆腐块）。改日志文案时**必须**跑 `tests/assets.rs`——
  它读真实字体查 `cmap`，缺字会直接让它失败（运行时只会静默变方块）。
  许可与体积取舍见 `assets/LICENSES.md`。
- **单位外观是 2D 纸片 + 贴地阴影**（`presentation/unit_sprite.rs`）：玩家 / 敌人用 2D
  精灵（绕 Y 轴对准相机的 billboard），**高度靠正下方地表上的黑色阴影表示**（离地越高
  阴影越小）。精灵与阴影是单位实体的子节点，因此单位根节点必须保持「脚底 + 无旋转 +
  无缩放」的参考系；树木 / 石头等装饰继续用 3D 模型。
- **功能不是领域**：像「战斗重置」这种只把已有系统拼一次的胶水，留在调用方
  （`spawn/restart.rs`），有数据模型 / 规则才进领域。
- 玩家输入只在 `input/` 翻译成消息（键盘 → `MoveCommand` / `FireCommand` /
  `MeleeCommand`），落盘由消息的消费领域负责；功能自带触发键跟着功能文件走。
- 位置一律用 Bevy `Transform`，不另造 `Position` 组件（避免两份坐标真相）。
- `world` 是纯数据域：**不引用渲染类型**（`Mesh3d` / `StandardMaterial` / `Assets<…>`），
  可用 `MinimalPlugins` 单测；网格化 / 材质一律放 `voxel_render`，两域只经区块消息通信。
- 单位与装饰的落脚高度取自 `world` 的地表函数，组装层不自己发明地形数据。

## 测试规范

- 单元测试写在源码旁的 `#[cfg(test)] mod tests` 中：A 原型共 152 个——`src/lib.rs` 的
  `mod tests` 放**整机用例**（25 个：真实流水线顺序 + `headless_app`），其余散在各领域
  文件里的纯逻辑用例；资产验收用例在 `tests/assets.rs`（2 个）；B 的领域层用例在
  `timeless-domain`。
- 测试名用描述性的 snake_case，例如 `layer1_speed_frame_decides_who_hits_first`。
- 使用 `assert_eq!`，断言意图不直观时附带简短说明。
- **不要用 `#[ignore]` 隐藏失败**：跳过的用例要么修好，要么在 `TODO.md` 写明根因与下一步。
- 提交前必须通过（A 在仓库根、B 在 `timeless/`，两棵树各自）：
  `cargo test` 全绿、`cargo clippy --all-targets -- -D warnings` 零警告、`cargo fmt --check` 通过。

## 提交与 Pull Request 规范

采用 Conventional Commits：`feat:`、`fix:`、`docs:`、`refactor:`、`test:` 前缀（如 `feat: add fireball skill`），每次提交只含一个逻辑变更。Pull Request 需说明改动内容与动机、关联根目录 `TODO.md` 中的里程碑条目，并记录手工验证结果（如控制台「谁先命中」输出）。
