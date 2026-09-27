# 表现契约（模拟端 ↔ 客户端）

> 🚧 **整篇是目标设计，代码里还没有。** 本文出现的**新**类型 / 字段一律标 🆕；
> 未标 🆕 的名字是**已存在**的，可以直接在 `src/` 里 grep 到（引用前请自行 grep 确认，
> 这是 [`docs/index.md`](../index.md) 第 3 条维护规则）。
>
> 本篇回答一个问题：**切开来之后，两侧怎么说话。**
> 谁裁决什么在 [01-architecture.md](01-architecture.md)；
> 怎么落地在 [04-migration.md](04-migration.md)。

## 零、三条纪律

1. **客户端只发「我想做什么」，不发「发生了什么」。**
   上行只有两类东西：**设备事实**（键按下、鼠标在哪）与**意图**（"用第 3 格技能"）。
   没有"我把敌人扣了 15 血"这种请求——扣血是模拟端的裁决。
2. **模拟端只发「现在是什么」，不发「你去画什么」。**
   下行是**数据视图**（谁在哪、还剩多久、谁在候场），不是显示指令
   （"把这个 Label 设成 `busy`"）。视图里没有像素、没有节点路径、没有动画曲线。
3. **每个跨边界的名字只有一处定义，且两侧各有一条对账测试。**
   键位只有一个真相（`src/input/keyboard.rs`）、HUD 文案只有一个真相（Rust 的 `model.rs`）、
   方块表只有一个真相（`world::VoxelType`）。第八节把这些对账写成机器可跑的测试。

⚠️ **为什么纪律 2 要写死**：现有 HUD 的三层分工是
`model.rs`（纯函数 + 快照，可单测）/ `scene.rs`（夹具）/ `system.rs`（取数 → 比对 → 写 UI）。
[`docs/backlog/hud.md`](../backlog/hud.md) 里那批「界面在说谎」的 bug（`ready`/`busy` 读反、
手动暂停仍显示 `RUNNING`）**全部出在语义层，而不是绘制层**。
把语义留在 Rust，等于把守这些 bug 的那批纯函数测试留下来。

## 一、上行 A：设备事件（Godot → Bevy 原生输入）

**做法：不发明协议，让设备事件长成 Bevy 本来就认识的样子。**

| Godot 事件 | 投递到 Bevy 的东西 | 消费方 |
| :--- | :--- | :--- |
| `InputEventKey` | `KeyboardInput { key_code, state, repeat, window }` | `ButtonInput<KeyCode>` → [`input`](../../src/input/mod.rs) 各系统 |
| `InputEventMouseButton` | `MouseButtonInput` | `ButtonInput<MouseButton>` → `pointer_click_input_system` |
| `InputEventMouseMotion` | `MouseMotion` + `CursorMoved` | `AccumulatedMouseMotion` → `camera_pan_input_system` |
| `InputEventMouseButton`（滚轮） | `MouseWheel { unit, x, y }` | `camera_zoom_input_system` |
| 窗口尺寸变化 | `Window` 的 `resolution` | `fit_ui_scale_system`（将随 HUD 一起删除） |

纪律（照抄 [AGENTS.md](../../AGENTS.md) 的「输入只翻译、不执行」）：

- 键位真相**仍在** `src/input/keyboard.rs`（`HotkeyBinds` + 各 `*_input_system`）。
  客户端**不认识任何游戏动作**：它不知道 `Q` 是火球，只知道"物理键 `KeyQ` 被按下"。
- **物理键码**（`KeyCode` / Godot 的 `physical_keycode`），与键盘布局无关。
  这条现在已经是仓库约定（[bevy-019.md](../bevy-019.md)「物理键优先」）。
- `player_move_input_system` 的**相机基**读取（`Query<&Transform, With<CameraRig>>`）
  在相机搬到客户端后走第二节的**相机镜像**，那条系统本体不动。

⚠️ **坑（两侧都要守）**：`just_pressed` 的语义依赖 Bevy 每帧在 `PreUpdate` 重建
`ButtonInput`。所以设备事件必须在**本次 `app.update()` 之前**投递完，且**同一帧内
按下 / 松开的顺序不许重排**（`press` 后紧跟 `release` 被合并成一帧，就等于这一帧
什么都没发生）。测试夹具里已经有这条纪律的痕迹：`src/lib.rs` 的 `press` / `release`
必须成对、连按两次要夹一次松手。

## 二、上行 B：姿态与 UI 事实

设备事件之外，客户端还必须报告三件**只有它知道**的事：

| 🆕 上行事实 | 落到模拟端的哪里 | 谁消费 | 为什么必须有 |
| :--- | :--- | :--- | :--- |
| **`CameraPose`**：相机世界变换 + 投影参数（正交 / 透视 + fov） | 写进 `CameraRig`（**已存在**的组件）的 `Transform`——保留它当镜像落点，`input` 一行不用改；`MainCamera` 随相机删除 | `input::player_move_input_system`（相机基）、`interaction::cursor_ray`（射线） | 方向键是**屏幕方向**，必须按相机朝向换算；拾取射线也由相机决定 |
| 🆕 `CursorPosition`：窗口内光标坐标 + 视口尺寸 | `Window::cursor_position()` 的镜像 | `interaction::hover_cell_system` | 拾取需要屏幕坐标 |
| `PointerOverUi`（**已存在**的资源） | 直接写资源 | `hover_cell_system` / `pointer_command_system` | "指针压在 UI 上"只有客户端知道 |

⚠️ **为什么是相机镜像，而不是"客户端算好世界方向再传"**：
「屏幕方向 → 世界 XZ 方向」这条规则现在**有测试**
（`camera_basis_makes_w_go_away_from_the_camera`、`vertical_camera_falls_back_to_world_axes`）。
让客户端算等于把规则复制一份到 GDScript，下次改机位就要改两处、且只有一处有测试。
镜像让规则留在原地：**客户端只报"相机在哪、朝哪"，不报"按上键该往哪走"。**

同样的理由适用于 `PointerOverUi`：它**是**输入层事实（"这一发点击被 UI 吃掉了吗"），
而不是裁决，所以由客户端回答是正当的——而且比现在用 `RelativeCursorPosition`
在 7 个区域根节点上算更准（现在是 Bevy UI 的近似：
`src/interaction/ui_capture.rs` 的文首解释了它为什么不用 `Interaction`）。

## 三、上行 C：UI 意图

Godot 的面板 / 按钮 / 拖拽**只能**发下表中已存在的消息。一张表看完
「哪些 UI 能做什么」：

| 🆕 意图名 | 对应的**已存在** Message | 客户端在什么时候发 |
| :--- | :--- | :--- |
| `select_skill` | `combat::attack::SelectSkill` | 点技能格、`Tab` 循环 |
| `cycle_skill` | `combat::attack::CycleSkill` | 只选不用的循环 |
| `use_skill` | `combat::attack::UseSelectedSkill` | 点技能格（热键路径）、`G` |
| `undo` | `timeline::UndoCommand` | 右键、撤下按钮 |
| `takeover` | `timeline::PlayerTakeover` | 任何"玩家这一帧自己动手了" |
| `wait` | `timeline::WaitCommand` | 空格、等待按钮 |
| `toggle_pause` | `clock::PauseRequest::Toggle` | 暂停按钮、`P` |
| `counter` / `abandon` | `combat::reaction::ReactionAnswer` | 威胁窗口上的"反制 / 忍" |
| `toggle_help` | `presentation::ToggleHelp` | 帮助按钮、`F1` |
| `reset_battle` | `spawn::ResetBattle` | 重开按钮、`F5` |
| `toggle_loadout` | `equipment::ToggleLoadout` | 装备面板、`T` |
| `move_to` / `attack_at` | `movement::MoveToCommand` / `UseSelectedSkill` | 点战场（经 `PointerCommand` 解释） |
| `primary_click` / `secondary_click` | `interaction::PointerCommand` | 战场左 / 右键 |
| `pan_camera` / `zoom_camera` | `presentation::PanCamera` / `ZoomCamera` | 中键拖拽 / 滚轮（客户端可自行处理，见下） |

三条补充规则：

1. **不进表的东西不许发。** 尤其禁止结果型请求：客户端**不许**发 `DamageEvent`、
   不许发 `combat::health` 的任何东西、不许直接改 `Health` / `Cell` / `DecisionSlot`。
2. **`PanCamera` / `ZoomCamera` 是"可以不上行"的两条**：相机已经是客户端的了，
   平移缩放完全可以在 Godot 里做完（`presentation::camera.rs` 的跟随与夹取逻辑照抄过去）。
   保留上行只是为了"相机夹取规则仍由 Rust 说了算"这一选项——**二选一要拍板**，
   别两边都改（见 [01-architecture.md](01-architecture.md) 第三节）。
3. **一条交互可能对应三条消息**：现在"按 `1`"就是 `SelectSkill` + `UseSelectedSkill`
   + `PlayerTakeover` 三条（`input::skill_menu_input_system`），"按 `Q`"也是三条
   （`player_skill_input_system`）。这个三连**现在散在三处**（数字键 / 热键 / 技能槽点击），
   加客户端就是第四处。落地时应收口成一个构造函数（🆕 `intent::use_slot(index)`），
   四边共用——否则"点技能栏"和"按数字键"迟早行为分叉。

## 四、下行 A：静态目录（握手时一次）

```text
客户端 ──Hello { protocol_version, client_build }──▶ 模拟端
模拟端 ──Directory { content_hash, voxels, abilities, slots, constants, texts }──▶ 客户端
```

| 🆕 目录段 | 来源（唯一真相） | 客户端拿它做什么 |
| :--- | :--- | :--- |
| `voxels` | `world::VoxelType::ALL`（名字 / 颜色 / 贴图键） | 建材质、贴图、UI 图例 |
| `abilities` | `skills::registry`（`AbilityDef`：id / kind / label / icon / 热键 / power / frame / 代价） | 建技能栏图标与 tooltip |
| `slots` | `equipment` 的槽位表 | 装备面板 |
| `constants` | `CELL_SIZE` / `CHUNK_SIZE` / `FOCUS_MAX` / `MAX_ENEMY_ROWS` / `WINDOW_SECONDS` | 布局、地面格绘制、圆点池 |
| `texts` | `presentation::hud::help::HELP_LINES`、日志模板 | 帮助面板（**文案在 Rust，客户端只排版**） |

- `content_hash`：任一侧的目录变了（加一个技能、加一种方块）就必须**改变哈希**，
  不匹配直接报错退出。⚠️ 静默错位是这套架构最贵的 bug：图标少一格、面板少一行，
  开发时看着像"没做完"，查起来要跨两个语言。
- **图标与贴图仍然是同一批文件**：`skills` 的 `icon_path` 指到的
  `assets/textures/ui/icon_*.png` 由两侧共用，`tests/assets.rs` 继续守
  「图标存在、是带 alpha 的方图」与「字体覆盖全部汉字」。客户端**不新增**一份资产真相。

## 五、下行 B：世界视图

实体身份：🆕 `ViewId(u64)`——会话内唯一、对客户端**不透明**的句柄。

> 为什么不用 Bevy 的 `Entity`：它是带世代的内部索引，暴露出去等于把引擎内部结构
> 写进协议（Bevy 升级、实体复用都会牵着走）。🆕 `ViewId` 由模拟端在实体生成时分配，
> 客户端只当键用（`Dictionary` / `HashMap`）。它是"实体 → Godot 节点"绑定的唯一钥匙，
> 也是将来做旁观 / 回放的天然锚点。

| 🆕 视图行 | 来源（已存在的组件 / 资源） | 客户端用途 |
| :--- | :--- | :--- |
| 单位 | `ViewId`、`Faction`、`Transform`、`Cell`、`UnitSprite` 对应的贴图键 | 纸片 + 贴地阴影 |
| 投射物 / 攻击实体 | `ViewId`、`Transform`、载荷类型（`Fireball` / `Arrow` / `MeleeShape`） | 飞行物与挥击表现 |
| 区块 | `world::chunk` 的坐标 + 🆕 网格数据 | `ArrayMesh` |
| 地面指示 | 🆕 由 `interaction::HoveredCell`、`combat::Threatens.cells`、AOE / 扇形预演算出的**格集合** | 悬停格、威胁格、预演贴片 |
| 装饰 | `presentation::decoration` 的摆放表（位置 + `assets/models/nature/*.glb` 名字） | `MultiMesh` / 场景实例 |
| 事件 | 🆕 `Spawned` / `Despawned` / `HitEffect` / `LogLine` / `Hint` | 生成销毁、粒子、日志、提示条 |

**增量从哪来（本设计里最 Bevy 原生的一点）**：
不要每帧拼全量快照，**复用 Bevy 自己的脏标记**——`Added<T>` / `Changed<T>` /
`RemovedComponents<T>` 就是现成的增量。现在的 `presentation` 已经在用同一个思想
的单机版（`TimelineCache` / `UnitPanelCache` 的 `matches()`：与上一帧一样就整帧不碰 UI），
把这套判据换成实体级增量即可，不必手写"上一帧副本比对"。

⚠️ **这一条在"同进程"形态下可以更省**：同进程时客户端直接读 `World`，
连序列化都省掉（见 [01-architecture.md](01-architecture.md) 第三节）。
但**数据形状要按可序列化的样子设计**，否则将来拆进程是重写而不是搬家。

## 六、下行 C：读数视图（HUD 的全部输入）

**结论：读数的语义留在 Rust，排版与绘制去 Godot。**

| 🆕 读数段 | Rust 侧已存在的产出 | 客户端只做 |
| :--- | :--- | :--- |
| 时间轴 | `presentation::hud::timeline::model::TimelineModel`（`state` / `lanes` / `ready` / `lane_faction`） | 画轨道、色块池、刻线、候场区 |
| 单位面板 | `presentation::hud::panels::model::UnitPanels`（`state_line` / `hp_text` / `focus_pips` / `insight_line` / `overflow_line`，含**排序与截断**） | 画面板行、条、圆点 |
| 技能栏 | `combat::attack::MenuSelection` + `can_cast` + `reaction` 的 `CounterSuggestion` | 四种状态（选中 / 悬停 / 买不起 / 能反制）用主题样式区分 |
| 日志 | `presentation::log` 的中文行（含 `[1.2s]` 前缀） | 列表 + 折叠 |
| 提示条 | `ActionBlocked` / `BlockRefused` / `EquipmentRefused` 的文案 | 淡出（用 `real_delta`，见第七节） |
| 帮助 | `HELP_LINES` | 排版 |
| 冻结状态 | `PauseReasons::labels()` + `ManualPause` → 🚧 `freeze_labels`（[`hud.md`](../backlog/hud.md) #50 待落地） | 状态行文本 |

⚠️ **为什么不让 Godot 从原始组件自己算读数**：这些文案里埋着**语义陷阱**与**排序规则**——
`ready` / `busy` 的判据（`is_idle()` 而非 `decided()`，读反过，见 #47）、
"敌人按离玩家最近排序 + 只画前 N 个 + 报溢出计数"、`EN -` 与 `FOCUS -` 的缺组件表示。
它们现在**都有纯函数单测**。搬到 GDScript 等于把这些测试丢掉，
并让"界面在说谎"重新变成可能——而这批 bug 正是本次改造的动机之一。

**代价（要承认）**：每帧要传一份读数（量很小：几百字节到几 KB），
且**改文案必须改 Rust 并重编译**（客户端不能就地改这句话）。
换来的是界面与规则同源：任何读数的改动都会同时被 Rust 的测试看见。

## 七、时钟契约

每一帧下行都带一个头：

```text
🆕 Frame {
  frame_index: u64,
  world_time:  f64,   // = Time<Virtual>::elapsed_secs()：世界时基
  real_delta:  f32,   // 这一帧的真实耗时：UI 时基
  frozen:      bool,  // = PauseReasons 非空 || ManualPause
  freeze_labels: [..] // 排序后的原因（含显示用的 manual）
}
```

**客户端因此有两套时钟。** 判据**不在本文**、也不许在这里复述：
那张「冻结时视觉纪律」表的**唯一真相源**是
[`docs/timeline.md`](../timeline.md) 第五节的
**「冻结时的视觉纪律：哪类元素走哪个时钟」**（2026-09-27 定稿）。

> **逻辑与"读世界状态"的视觉走 `world_time`；
> 只服务于"人机交互"的视觉走 `real_delta`。**

客户端这一侧只是**换名字**（`Time<Virtual>` → `world_time`、`Time<Real>` → `real_delta`），
逐行归类与理由见那张表，**不在这里重复**——同一张表写在两处必然漂移
（当天刚在字体字表上吃过一次这个亏：两份清单不同步，屏幕上出现豆腐块）。
两处仍需各自留意的差异只有一点：

- **投射物 / 位移插值**在这里是 `world_time`，而且**必须**是——位置由模拟端按虚拟时间
  积分下发，客户端若拿真实时间插值，就会与权威状态**错位**（不只是"看起来不对"）。

⚠️ **不许用 `real_delta` 补世界动画**（哪怕只是想"看起来没卡住"）：
世界冻结是**玩法的一部分**（玩家正在读盘、正在决定），让画面偷偷动就是在骗玩家
——这和「时间轴显示 `RUNNING`」是同一类 bug，只是更难发现。

## 八、对账测试（防漂移的机器）

契约要能被**机器**检查，否则两周后两侧就会各说各话。照仓库既有的
「文案对账」手法（`the_help_says_space_is_a_wait_and_p_is_the_pause` 那一族）扩大：

| 契约文件（🆕，建议住 `contract/`） | Rust 侧测试 | Godot 侧测试 |
| :--- | :--- | :--- |
| `view.v1.json`：读数视图与视图行的**字段路径表** | 序列化结果**恰好**是这些键（多一个少一个都红） | 每个键都有节点 / 属性绑定（少一个就红） |
| `intents.v1.json`：上行意图名 + 负载字段 | 每个意图都能映射到**已注册**的 `Message`（表里有、`add_message` 里没有 → 红） | 每个意图都至少被一处"发送器"引用（表里有、没人发 → 红） |
| `directory.v1.json`：静态目录形状 | `Directory` 的字段与 `VoxelType::ALL` / `SkillRegistry` / 槽位表一致 | 握手后每个条目都能建出节点 |
| `assets` 清单 | `tests/assets.rs`（**已存在**）：字体覆盖汉字、图标是带 alpha 的方图 | 同一批文件在 `res://` 下可见（路径对账） |

**不需要对账的**：键位。因为设备事件原样转发，键位真相只有 `src/input/keyboard.rs` 一处。
⚠️ 唯一的例外：如果 Godot 侧要给自己的 UI 加键盘快捷键（面板开合之类），
必须有一条扫描测试保证**它不与游戏键冲突**——否则会出现"按 `Tab` 既切技能又切面板"
这种以后很难查的 bug。

## 九、不变量（破坏了这套架构就散了）

1. **客户端永不裁决**：不改血、不掷骰、不算伤害、不判可行走性、不改 `DecisionSlot`。
2. **模拟端永不认识 Godot 类型**：`Node` / `Vector3` / `Resource` / 信号不许进
   `src/` 的任何领域层（同「`world` 不引用渲染类型」那条铁律的理由：无头单测与拆进程
   都靠它）。
3. **一个事实只有一处定义**：文案在 Rust、排版在 Godot；键位在 Rust、UI 快捷键在
   Godot 且不重叠；方块表在 Rust、材质在 Godot。
4. **表现可以晚一帧，裁决不行**：客户端渲染滞后一帧无害（世界大部分时间冻着，
   玩家看的是"冻结的那一帧"）；但任何"客户端先动、模拟端后追认"的设计都不许有。
