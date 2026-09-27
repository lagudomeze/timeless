# Godot 客户端：节点树、场景与渲染

> 🚧 **整篇是目标设计，代码里还没有。**
> 本篇回答一个问题：**Godot 那一侧长什么样**（目录、节点、数据绑定、渲染与输入）。
> 边界在 [01-architecture.md](01-architecture.md)，两侧怎么说话在 [02-contract.md](02-contract.md)。
>
> ⚠️ 本节的 Godot API 名以官方文档为准，**动手前先核对**（同 [`bevy-019.md`](../bevy-019.md)
> 的规矩：禁止用记忆充当权威）。第八节列了要先核对的那几处。

## 一、项目与目录约定

```text
client/                          # 一个 Godot 4 项目（与根 package `app` 并列，不是 workspace 成员）
├── project.godot                 # 项目设置：分辨率策略、渲染、自动加载
├── timeless.gdextension          # 指到 Rust 动态库（debug / release 两个 profile）
├── rust/                         # 🆕 gdext 宿主 crate（cdylib，path 依赖 ../）
│   ├── Cargo.toml                #   `app = { path = ".." }` + `godot = "..."` + crate-type = ["cdylib"]
│   └── src/lib.rs                #   🆕 SimHost：唯一的跨界类型
├── scenes/                       # .tscn —— 在 Godot 编辑器里拖出来
│   ├── Main.tscn                 #   根：SimHost + World + Hud 三层
│   ├── world/{Terrain.tscn, Unit.tscn, GroundOverlay.tscn, Decor.tscn}
│   └── hud/{Timeline.tscn, PlayerPanel.tscn, EnemyRow.tscn, SkillBar.tscn,
│            CombatLog.tscn, ActionHint.tscn, HelpPanel.tscn}
├── scripts/                      # GDScript —— 胶水（拿视图数据写节点）
│   ├── sim_bridge.gd             #   唯一与 SimHost 说话的地方（其余脚本只读 bridge.view）
│   ├── world/…  · hud/…
├── theme/timeless.tres           # 统一字体 / 字号 / 颜色 / 面板样式（含技能栏四态）
└── tests/                        # Godot 侧测试（见第七节）
```

**两条硬规矩**

1. **只有 `sim_bridge.gd` 与 `SimHost` 说话**。其他脚本不许直接调宿主——
   否则"跨界点唯一"这条就没了，将来换 IPC（[01](01-architecture.md) 形态丙）要改几十个文件。
2. **`rust/` 是唯一的 Rust↔Godot 层**。根 package `app` 的依赖树里**不许**出现 `godot`
   （[01](01-architecture.md) 第六节：让 Cargo 依赖图强制"领域层零 Godot"）。

⚠️ **资产怎么共用**（Godot 的 `res://` 必须在项目目录内，而资产在仓库根的 `assets/`）：
两种方案与代价见 [05-risks.md](05-risks.md) 的 E1；**建议**在 `client/` 里用目录联接
暴露 `res://assets`，并加一条"两侧看到同一批文件"的对账测试。

⚠️ **构建面**：`client/rust/` 是**独立 package**（path 依赖根 package `app`），
所以它有自己的 `Cargo.lock` 与 `target/`。两条要记的事写在 [05](05-risks.md) E2：
① bevy 会被编两遍（想省就共享 `CARGO_TARGET_DIR`，代价是抢 target 锁）；
② 两个 lockfile 的 bevy 版本**必须同步升级**。

## 二、`SimHost`：唯一的跨界节点

一个 Rust 类（gdext），挂在 `Main.tscn` 的**第一个**子节点上。对外只有四样东西：

| 🆕 成员 | 方向 | 说明 |
| :--- | :--- | :--- |
| `step(delta)` | GDScript → Rust | 推进模拟一帧（内部就是 `App::update()`，[01](01-architecture.md)） |
| `send_intent(name, payload)` | GDScript → Rust | 上行意图（表见 [02](02-contract.md) 第三节） |
| `send_key / send_mouse / send_cursor` | GDScript → Rust | 设备事实（[02](02-contract.md) 第一节） |
| `view_updated(frame)` 信号 | Rust → GDScript | 每帧视图（世界行 + 读数 + 时钟头） |

**六条实现纪律**

1. **事件在 `step()` 之前投递完**（同帧内不许重排按下 / 松开——[02](02-contract.md) 第一节的坑）。
2. **用信号 push，不让 UI 每帧 pull**：pull 会让"谁先跑"依赖场景树顺序；
   push 在 `_process` 里当场发，UI 同帧更新。
   ⚠️ **但信号在热重载后会断线**：gdext 的 typed signal 走自带 vtable 的自定义 `Callable`，
   热重载后那些函数指针失效（官方原话是 **UB，通常表现为崩溃**），所以 gdext 会在重载前
   **自动断开全部 typed 连接**。两条对策：① 连接用**标准 Callable**
   （`Callable::from_object_method` / `Gd::callable()`，它能活过热重载）；
   ② 或监听 `ObjectNotification::EXTENSION_RELOADED` 在重载后重连。
   （来源见 [README](README.md) 3.1 那张表的"信号与热重载"行。）
3. **宿主不碰节点树**：`SimHost` 只产出 `Dictionary`，一个 `Node3D` 都不创建、不写属性。
   这样"模拟端永不认识 Godot 类型"这条不变量在宿主里也只是**一个文件**的例外。
4. **所有 Godot 触碰都在主线程**（Godot 的硬规矩：SceneTree 操作只能在主线程）。
   本设计的纪律是：宿主只在 `_process` / `_input` / `_unhandled_input` 回调里跑；
   重计算（区块网格化）留在 Bevy 的任务池里并行，但**只回传纯数组，绝不回传 Godot 对象**。
   ⚠️ 这条纪律有一个直接收益：**有机会不打开 gdext 的 `experimental-threads`**
   （官方自述 "high risk of unsoundness"）——能不能不开，是 P−1 spike 的验项。
5. **panic 要兜住，但要知道兜不住哪里**：gdext 装了 panic hook，**回调里的 panic 多数被捕获**
   并以错误形式打进 Godot 面板（游戏继续）；但 **`Drop` 里的 panic 是非 unwind 的 abort**，
   double borrow 之类也会走到 abort —— 那会**连编辑器一起带走**。
   所以宿主入口自己再包一层 `catch_unwind`（`godot-bevy` 就是这么做的），
   并且**不要在 `Drop` 里做可能 panic 的事**。
6. **日志统一出口**：Bevy 的 `bevy::log` 与 Godot 的 `print` 会分成两个窗口，
   实机排查时最难受的就是这个。`godot-bevy` 专门做了 `GodotBevyLogPlugin` 把 Bevy 日志
   接到 Godot 控制台——**照抄这个思路**（[05](05-risks.md) B5）。

## 三、世界渲染

### 3.1 地形（区块 → `ArrayMesh`）

- 数据形状：🆕 `ChunkMesh { coord, origin, surfaces: [ { voxel, positions, normals, uvs, colors, indices } ] }`
  （每个 `surface` 对应一种 `world::VoxelType`，顺序与 `VoxelType::ALL` 一致）。
- 网格**计算**仍在 Rust（`build_chunk_meshes` 的贪婪合并 + 顶点 AO，12 条单测），
  客户端只负责"接收数组 → 建 Mesh"。
- 每区块**一个** `ArrayMesh`（一种方块一个 surface）+ 一个 `MeshInstance3D`；
  ⚠️ 不要把所有区块塞进同一个 `ArrayMesh` 的多个 surface（多 surface = 多材质槽 /
  多 draw call，且无法单独剔除与释放）。
- `ArrayMesh.add_surface_from_arrays(PRIMITIVE_TRIANGLES, arrays)` 的**数组下标**是固定的：
  `0` 顶点（`PackedVector3Array`）、`1` 法线、`3` 顶点色（`PackedColorArray`）、
  `4` UV（`PackedVector2Array`）、`12` 索引（`PackedInt32Array`）；
  `arrays` 要 `resize(Mesh.ARRAY_MAX)`，只有**自定义通道**才需要自己算 `flags`。
  （来源：[ArrayMesh 教程](https://docs.godotengine.org/en/4.5/tutorials/3d/procedural_geometry/arraymesh.html)）

> ⚠️⚠️ **最容易让"地面整片看不见"的一条：绕序方向相反。**
> **Godot 的正面是顺时针（clockwise）**（ArrayMesh / SurfaceTool 文档都写了），
> 而 Rust 侧生成的是**从外侧逆时针**——有测试钉着：
> `every_greedy_quad_winds_counter_clockwise_from_outside`。
> 直接上传的结果是**整片地形被背面剔除**（症状："地面没了"或"从下面才看得见"），
> 而顶点数、面积、AO 全都对，所以极难往绕序上想。
>
> **对策（三选一）**：
> ① **上传时反转每个三角形的索引顺序**（推荐：一次循环，数据形状与 Rust 侧测试都不动）；
> ② 材质 `cull_mode = CULL_DISABLED`（双面渲染，代价是背面剔除失效、透明与光照受影响）；
> ③ 在 `build_chunk_meshes` 阶段就翻过来，并改那条测试的判据（会动到网格化测试）。
>
> **P1 的第一件事**就是拿一个孤立方块验"六个面都能看见"，再上整片地形（[05](05-risks.md) C2）。

- ⚠️ **顶点色必须当 albedo 用**（Godot 4：`BaseMaterial3D.vertex_color_use_as_albedo = true`），
  否则面朝向与 AO 的明暗全丢——症状也是"网格坏了"，但实际是材质设置（[05](05-risks.md) C1）。
- ⚠️ **UV 的跨度是"格数"**（贪婪合并出来的矩形跨 N 格，`uv_extent` 最大到 `CHUNK_SIZE`），
  所以材质纹理要 repeat。**过滤设置的作用域要分清**：
  `rendering/textures/canvas_textures/*` 只管 2D/canvas，**管不到 3D**；
  3D 侧要逐材质（`BaseMaterial3D.texture_filter`）或逐节点
  （`SpriteBase3D.texture_filter`，默认是带 mipmap 的线性过滤）设 nearest，
  也可以改贴图的导入选项（影响全局，慎用）——[05](05-risks.md) C3。
- 区块生命周期：视图里区块出现 / 标脏 / 消失 → 建 / 换 / `queue_free`；
  `MeshInstance3D` 用池，别每帧新建。上传回主线程（`call_deferred`，C6）。
  实例化渲染节点**默认不是线程安全的**，所以"worker 线程算数组 → 主线程建 Mesh"是唯一稳妥的姿势。
- 两个可选优化（值得实测）：给 `MeshInstance3D` 正确的 `custom_aabb`（否则视锥剔除会误杀，
  对 shader 里偏移顶点的尤其重要）；区块局部更新用
  `ArrayMesh.surface_update_vertex_region(...)`（需 `ARRAY_FLAG_USE_DYNAMIC_UPDATE`）。

### 3.2 单位（纸片 + 贴地阴影）

- `Unit.tscn`：一个面向相机的 `Sprite3D` + 一个贴地阴影片。
  ⚠️ 单位根节点在 Bevy 侧有约定「脚底 + 无旋转 + 无缩放」（[`unit_sprite.rs`](../../src/presentation/unit_sprite.rs)），
  **这个约定要继承过去**：视图里的 `Transform` 就是脚底位置，客户端写 `global_position` 时不加偏移。
- ⚠️ **`billboard` 默认是 Disabled，必须显式设**；而且要的是
  **`BaseMaterial3D.BILLBOARD_FIXED_Y`（只绕 Y 轴）**，不是 `BILLBOARD_ENABLED`
  （后者连俯仰一起对齐，斜视角下纸片会"翻倒"）。正交相机下没有任何"自动面向相机"的行为。
- 像素风建议 `alpha_cut = ALPHA_CUT_DISCARD`：硬边、**天然免疫透明排序问题**，与像素风契合；
  要柔边再考虑 `ALPHA_CUT_OPAQUE_PREPASS`（较慢但排序正确）。
  ⚠️ 用 `modulate` 提亮是无效的（3D sprite 不支持 >1 的过曝），高亮请用 emission 或描边。
- **贴地阴影优先"略微抬高的透明 quad"**；`Decal` 虽然被官方点名适合 blob shadow，
  但有三条硬限制：**Compatibility 渲染方式不支持**、Mobile 下每个 mesh 最多 8 个
  （超出会随相机闪烁）、**过滤是全局项目设置**（`rendering/textures/decals/filter`）。
  只有"阴影必须贴合台阶 / 起伏地形"时才值得用它，并用 `cull_mask` 防止糊到单位身上。
- 阵营辨识是**已知待办**（[`docs/backlog/presentation.md`](../backlog/presentation.md) #56：贴地阵营环）：
  在 Godot 里它就是一个 `MeshInstance3D` 圆环——**这次搬家顺手把它做出来**，
  比在 Bevy 里补一遍再搬更省。
- 装饰物（`assets/models/nature/*.glb`）：Rust 给摆放表（位置 + 名字），客户端实例化。
  ⚠️ 几十个用普通节点就够；`MultiMeshInstance3D` 的收益在几百~几千个，且有几条硬规矩：
  `mesh` 必须是 **Mesh 资源**（`Sprite3D` 要用 `SpriteBase3D.generate_triangle_mesh()` 转）、
  `use_colors` / `use_custom_data` **必须在 `instance_count` 之前设好**（之后再设无效）、
  AABB 必须自己给（`custom_aabb`），而且整个 MultiMesh 在空间上算**一个**对象
  （实例彼此离太远反而掉性能）。

### 3.3 地面指示（悬停 / 威胁 / AOE / 扇形）

- 一个 `GroundOverlay` 节点：**每格一个小面**的池化 quad（或上百格时用 `MultiMeshInstance3D`）。
  材质用 `SHADING_MODE_UNSHADED` + 透明 + 视需要 `cull_mode = CULL_DISABLED`。
- 输入是**格集合 + 种类**（🆕 `overlays: [{ kind, cells: [...] }]`）：
  `hover` / `threat` / `aoe` / `cone` / `path`。
- ⚠️ **哪些格**是裁决（`combat::targeting` 的形状相交、`Threatens.cells`），
  由 Rust 算；**怎么画**是表现，归客户端。
- ⚠️ **z-fighting 的正解是几何抬升**（沿世界 Y 抬 0.01~0.05，体素格尺度下足够），
  或放进独立渲染层；`render_priority` **只排"透明物体之间"的先后，不解除深度冲突**。
  另外透明物体是"整个对象"参与从后往前排序的，所以**一整块大平面**会与纸片单位闪，
  拆成每格一个小面更稳。
- ⚠️ 多个指示同时出现时要有**视觉优先级**（现在由 `interaction::visual.rs` 的池化与显隐决定）：
  搬过去的时候把这条规则也搬过去，别让它变成"谁后画谁在上"（[05](05-risks.md) C4）。
- 需要"贴合起伏地形的地面染色"时才上 `Decal`（限制见 3.2），并用
  `upper_fade` / `lower_fade = 0` + 压扁 Y 尺寸来收紧 AABB。

### 3.4 特效（粒子 / 浮字 / 动画）与"由外部时钟驱动"

- **跟随外部虚拟时间的官方入口是 `AnimationMixer`**：把
  `AnimationPlayer` / `AnimationTree` 的 `callback_mode_process` 设为 **MANUAL**，
  然后每帧用 Rust 给的 `world_delta` 调 `advance(delta)`。
  （4.5 起 `AnimationPlayer.set_process_callback()` 已 deprecated，改用基类
  `AnimationMixer.callback_mode_process`。）
  ⚠️ **在根节点统一 advance**，别让多个节点各自推进同一帧，否则同一帧被推进两次。
- 命中粒子用 `GPUParticles3D`，位置由事件给；**时间基是 `world_time`**（[02](02-contract.md) 第七节）。
  ⚠️ **不要让粒子跟着 Godot 自己的时钟跑**：世界冻结时粒子必须**定格在"刚打中"**
  （现有 `presentation/effects.rs` 的设计意图，见 [`docs/backlog/presentation.md`](../backlog/presentation.md)）。
  简单做法：冻结时把粒子的 `speed_scale` 置 0；严格做法：自定义 shader 的
  `time` 参数直接吃 `world_time`。
- 伤害浮字（backlog 那条未做项）同理：0.6s 上飘淡出，但**用 `world_time` 计**，
  这样冻结期间它也定格、玩家解冻后看完。

> ⚠️⚠️ **两条绝对不要用的"暂停"**：
>
> 1. **`get_tree().paused = true`**：它会把**宿主自己的 `_process` 一起停掉**
>    （`Pausable` 节点在暂停期间不再收到 `_process` / `_input`），
>    于是模拟再也不被推进 —— **世界再也解冻不了**。它还会顺带停掉动画 / 粒子 / 音频。
> 2. **`Engine.time_scale`**：它会缩放所有 delta（连 UI 的 `_process` 动画一起变慢），
>    而"冻结 / 慢动作"（含将来的命中定帧）是**模拟端 `Time<Virtual>` 的事**
>    （[`docs/backlog/clock.md`](../backlog/clock.md)）。
>
> 本方案里"世界冻结"**只**由下行的 `world_time` / `frozen` 表达：客户端照常 `_process`、
> 照常收输入，只是**不推进任何世界表现**（[02](02-contract.md) 第七节的两套时钟）。

## 四、输入

| 事项 | 做法 | ⚠️ 坑 |
| :--- | :--- | :--- |
| 键盘 | `_unhandled_input` 收 `InputEventKey`，用 **`physical_keycode`**（布局无关），原样转发 | 键位真相仍在 `src/input/keyboard.rs`，客户端不认识"火球" |
| 鼠标按键 / 滚轮 | 同上转发 | 中键拖拽平移相机 = 客户端自己做（若拍板，见 [01](01-architecture.md)） |
| 光标 | 每帧把视口内坐标转发 | 窗口缩放 / 全屏切换后坐标要对得上（同一窗口，风险低） |
| **UI 是否吃掉指针** | 用 Godot 的 hover 判定报 `PointerOverUi`；点击用 `_unhandled_input`（UI 已经先处理过） | ⚠️ 这条让"点面板不走角色"**天然成立**——但**必须**给 Rust 报 `PointerOverUi`，否则悬停高亮与预演会留在世界里（[`playtest-checklist.md`](../playtest-checklist.md) 第 3 节） |
| **UI 焦点** | 所有 `Control` 的 `focus_mode` 设 none；面板背景 `mouse_filter = STOP`、纯装饰 `IGNORE` | ⚠️ Godot 默认能用 **方向键 / Tab 做 UI 导航**：一旦某个控件拿到焦点，方向键会被 UI 吃掉（玩家按上键不动），Tab 会去切焦点（而 Tab 是游戏的循环技能键）。**P1 就要把这条验掉** |
| 相机 | 客户端的 `Node3D`（跟随 + 夹取） | 姿态要**镜像给 Rust**（[02](02-contract.md) 第二节）：方向键按屏幕方向走靠它，拾取射线也靠它 |
| 暂停 | 不用 `get_tree().paused`、不用 `Engine.time_scale` | 冻结是模拟端 `Time<Virtual>` 的事（[05](05-risks.md) B6） |

**Godot 的输入传播顺序**（官方逐条编号，决定了我们该挂哪个回调）：

```text
_input()  →  GUI（Control._gui_input()）  →  _shortcut_input()  →  _unhandled_key_input()  →  _unhandled_input()
             ↑ mouse_filter 决定收不收、传不传
```

- 官方对游戏输入的建议原话是：**`_unhandled_input()` 通常更合适，因为它允许 GUI 先拦截事件**——
  这正是我们要的"点面板不走角色"。
- ⚠️ 但有一条**容易踩的细节**：GUI 的**键盘**事件**不沿场景树上行**——只有收到它的那个 `Control`
  能处理，处理不掉才会作为非 GUI 事件继续传到 `_unhandled_input()`。
  所以"某个面板获得焦点后按空格"可能被面板吃掉。**把所有 Control 的 `focus_mode` 关掉**是最省事的解法。
- ⚠️ **别用 `InputMap` 表达游戏动作**：官方写明这个单例**不会被保存**，它的内容来自项目设置
  （`input/<action>` 键），动态重映射要开发者自己找地方存。本方案的键位真相在 Rust，
  客户端只需要"物理键码进、物理键码出"，**根本用不到 InputMap**（只在给客户端自己的 UI 快捷键时才用）。
- ⚠️ **给玩家看的键名要和"物理键"区分开**：`physical_keycode` 是"101/102 键 US QWERTY 上的位置"，
  而 `keycode` / `key_label` 才是"键上印的字"。现在的 `HELP_LINES` 走的是英文键名
  （`Space` / `P` / `F5` …），在非 QWERTY 布局下与实际物理键不符——
  搬过去时可以顺手改成"客户端按当前布局生成显示文本"（`OS.get_keycode_string` / `key_label`）。

**哪三个键属于"客户端 UI 快捷键"**（可以不进游戏意图表）：帮助面板开合、日志折叠、
面板滚动。⚠️ 它们**不得与游戏键冲突**（[02](02-contract.md) 第八节的扫描测试）。
一个具体的分叉点：`F1` 现在是游戏意图（`input::player_help_input_system` → `ToggleHelp`）。
**建议**：把它划给客户端（"面板开合"是纯 UI 事实），`HELP_LINES` 随静态目录下发；
`presentation::hud::ToggleHelp` 与 `help.rs` 的消费者随之删除。

## 五、HUD：一块一块对应

下表左边是 Godot 场景，中间是**现在**的 Bevy 节点名（`presentation/hud/layout.rs` 的
`setup_hud_names_every_region_it_builds`，40+ 个具名节点就是"HUD 有哪几块"的验收表），
右边是数据来源（[02](02-contract.md) 第六节）。

| 🆕 Godot 场景 | 现有 Bevy 节点 | 数据来源（Rust 侧产出） |
| :--- | :--- | :--- |
| `Timeline.tscn` | `Timeline` / `TimelineState` / `TimelineRows` / `TimelineLanes` / `TimelineLaneLabels` / `TimelineStaging` / `TimelineTick*` / `TimelinePlayhead` / `TimelineBlock*` / `TimelineReady*` | `hud/timeline/model.rs` 的 `TimelineModel`（状态行 / 车道 / 色块 / 候场） |
| `PlayerPanel.tscn` | `PlayerPanel` / `PlayerPortrait` / `PlayerInfo` / `PlayerStateLine` / `PlayerHp*` / `PlayerEnFill` / `PlayerFocus*` / `PlayerAction` | `hud/panels/model.rs` 的 `UnitPanels`（`state_line` / `hp_text` / `focus_pips` / `insight_line`） |
| `EnemyRow.tscn`（行池） | `Enemy1Row..Enemy3Row` / `EnemyOverflow` / `EnemyPanels` | 同上（含**排序**与**截断 + 溢出计数**） |
| `SkillBar.tscn` | `SkillBar` / `SkillSlot0..3` / `SkillTooltip` | `combat::attack` 的 `MenuSelection` + `skills` 目录 + `reaction` 的 `CounterSuggestion` |
| `CombatLog.tscn` | `CombatLog` / `CombatLogHeader` / `CombatLogBody` | `presentation/log.rs` 的条目文本（含 `[1.2s]`） |
| `ActionHint.tscn` | `ActionHint` / `ActionHintText` | 提示事件（`ActionBlocked` 等的文案；淡出用 `real_delta`） |
| `HelpPanel.tscn` | `HelpPanel` / `HelpTitle` / `HelpKeys` | `HELP_LINES`（静态目录） |

**布局与主题**

- 五块的位置不变（顶部时间轴 / 左下玩家面板 / 右下敌人面板 / 底部技能栏 /
  右下偏上日志 + 居中帮助），用 `Control` 的锚点 + 容器（`MarginContainer` /
  `VBoxContainer` / `HBoxContainer`）表达，**不再需要手算像素 + 百分比锚点**
  （这是改用 Godot 最直接的收益之一）。
- 分辨率适配：⚠️ **Godot 没有"按窗口高度等比缩放整套 UI"的单一开关**（现有的
  `fit_ui_scale_system` 是自算的）。官方文档给的两条路：

  | 方案 | 设置 | 结果 |
  | :--- | :--- | :--- |
  | **甲（推荐先试）** | `display/window/stretch/mode = canvas_items` + `aspect = keep_height` | 3D 不受影响；视口高度锁在 base height（缩放因子 = 窗口高 / base 高），**更宽时横向看到更多**——与"HUD 贴上下边"的现有观感最接近 |
  | 乙 | 同上但 `aspect = keep_width` | 官方称它「通常是最适合可缩放 GUI/HUD 的选择，这样一些 Control 可以锚到底部」——若你更希望**横向固定、纵向伸缩**就选它 |
  | 丙 | `mode = disabled` + 自己在根 `Control` 上按 `window.size.y / BASE_HEIGHT` 设 `scale` | 与现有逻辑等价，但适配代码要自己写；UI 与 3D 的缩放可以完全分开 |

  `BASE_HEIGHT` / `MIN_UI_SCALE` / `MAX_UI_SCALE` 从 `Directory.constants` 取，别在两处各写一个数
  （[05](05-risks.md) D2）。像素风还需要 `stretch/scale_mode = integer`（4.2 起），
  并注意 `gui/theme/default_theme_scale` **只在启动时读一次**、运行时改不了。
- 统一字体与字号走 `theme/timeless.tres`（一个 `Theme` 资源挂在 autoload 根节点上，全树继承；
  同类不同外观用 `theme_type_variation`）。**文案的铁律不变**：
  HUD 用英文、日志正文用中文——而且它现在更容易执行，因为文案仍由 Rust 生成，
  `tests/assets.rs` 的逐字 `cmap` 验收继续守着字体覆盖。
  ⚠️ **中文正文用动态字体、不要用 MSDF**：MSDF 的强项是"一套字体跨字号清晰"，
  但它对**密集细笔画 + 小字号**容易糊/缺笔画，而中文正好笔画密。标题之类需要大幅缩放的再用 MSDF，
  并给 `FontFile.fallbacks` 配缺字回退（[05](05-risks.md) D1）。
- 技能栏四态（选中 / 悬停 / 买不起 / 能反制）用 `StyleBox` + 主题变体区分；
  **判据来自 Rust**（买不起 = `can_cast` 的 `BlockReason`；能反制 = `CounterSuggestion`）。
- 数字对位（`HP 50 / 100`、`cell (1, 0)`）交给字体或固定宽度容器，
  **不许**让 Rust 文案补空格（[`hud.md`](../backlog/hud.md) #51 的老坑）。

**交互**

- 技能格**点击 = 只选中**（现在就是这么拍板的，见 [`hud.md`](../backlog/hud.md) #55）；
  "选中 + 用一次 + 接管"这个三连（现在散在数字键 / 热键 / 技能槽三处）应收口成
  一个构造函数（[02](02-contract.md) 第三节第 3 条），客户端只是第四个调用方。
- 日志折叠、面板开合：纯客户端状态（不发上行意图）。
- 时间轴悬停 → 读数 tooltip（`hud/timeline/readout.rs` 的文本，逐字符一致）；
  同时要在战场上圈出那个单位（现在的 `TimelineFocusRing`）——`ViewId` → 节点的反查是它的实现。

## 六、表现由时间线驱动（搬迁后仍然成立的三个好性质）

1. **一条行动自带它何时落地**：`ScheduledAction.execute_at` + `ActionTiming.windup / recovery`
   就是画色块所需的全部信息（现在的时间轴正是这么画的：色块 = 占用，刻线 = 结算）。
   客户端因此**不需要预测**、不需要"猜对方的动画进度"。
2. **冻结时世界定格**：客户端不需要用真实时间补任何世界动画（[02](02-contract.md) 第七节）。
3. **表现可以晚一帧**：世界大部分时间冻着，玩家看到的就是"冻结的那一帧"，
   所以宿主 step → 视图 → 节点写入这一条链路上多一帧延迟是**无害**的。

## 七、调试与测试

| 手段 | 用途 |
| :--- | :--- |
| `--dump-view` 离线快照（[04](04-migration.md) 第零节） | 客户端**脱离模拟端开发**：喂文件即可渲染一帧；也是回归夹具 |
| Godot 远程调试 + 视图字典打印 | 客户端侧问题（节点没更新、绑定错了） |
| **LLDB attach 到游戏进程** | Rust 侧断点。⚠️ 官方示例故意**不给 `-e`（编辑器模式）**，注释写着"编辑器不保留断点"——实际工作流是从命令行/VS Code 起**游戏**再 attach；且只有 Rust 帧有符号（除非自己编一份 debug 版 Godot） |
| BRP（模拟端仍然开着） | 模拟端侧问题：`world.query` 读决策槽 / 冻结原因 / 血量。⚠️ 截图类方法失效（无渲染），换成 Godot 截图 |
| **Godot 侧三类测试** | ① 场景契约（节点名与绑定齐全）；② 离线快照渲染（断言读数文本与快照一致）；③ 意图发送器全被引用（[02](02-contract.md) 第八节） |
| 固定机位截图脚本 | 与 Bevy 版并排对照（[04](04-migration.md) P1 的退出标准） |

**CI / 无头运行的确凿命令**（官方命令行文档）：

| 命令 | 说明 |
| :--- | :--- |
| `--headless` | 等价于 `--display-driver headless --audio-driver Dummy`；官方点名"与 `--script` 搭配使用" |
| `-s <script>` | 脚本**必须继承 `SceneTree` 或 `MainLoop`**（不是随便一个 `.gd`）；可给资源路径或绝对路径 |
| `--check-only` | 只解析错误然后退出（配 `-s` 做语法检查） |
| `--import` | 启动编辑器、等资源导入完成、退出。**CI 的第一步**（没有导入缓存时加载会失败） |
| `--quit-after <N>` / `--fixed-fps <fps>` | 跑 N 次迭代 / 固定帧率，用来做确定性回放 |
| `--write-movie <file>` | 把帧写成视频或 **PNG 序列**（强制 `--fixed-fps`）——"截图对账"的官方入口 |
| `--path <dir>` | 项目目录（需含 `project.godot`）；⚠️ 导出时的相对路径是**相对 project.godot，不是 cwd** |

⚠️ 三条已知坑：① 无 GPU 的 CI 上**必须** `--headless`，而且 headless 下
`Engine.get_frames_drawn()` **恒为 0**——所以"像素级断言"在无头环境里做不到；
② 导出要求 `export_presets.cfg` 里的 preset 名完全一致、**目标目录必须已存在**；
③ 第三方测试框架（GUT / GdUnit4）的命令行用法**本次调研没查到权威来源**，
落地时按各自官方文档核对后再写进 CI（**别凭记忆写**）。

> 💡 对体素项目更稳的"测试"是**不比对像素**：在 headless 里断言"渲染输入"
> （顶点/索引数组、MultiMesh 缓冲、材质属性、Control 布局矩形、读数文本），
> 把"好不好看"交给人看（[05](05-risks.md) E5）。

## 八、动手前仍要核对的地方

> 依据 [`bevy-019.md`](../bevy-019.md) 的规矩：**禁止用记忆充当权威**。
> 本节原本列了 7 条待核对项；调研后其中 5 条已经查证并写进上文（绕序与数组格式、
> 顶点色、输入与焦点、分辨率设置、CJK 字体），**剩下的几条要在动手前核**：

1. **目标 Godot 版本要钉住再核**：本次查证的类文档是 **4.4 / 4.5**
   （`/en/stable/` 现在已是 4.7），而 `godot-bevy` 0.12.x 的主 target 是 **Godot 4.6.x**、
  gdext 0.5 默认 API 级别也是 **4.6**。落地时把版本定在 4.6，
   然后用 `/en/4.6/` 的文档复核本文引用的结论（尤其 4.5→4.6 的迁移项）。
2. **`Control.mouse_filter` 的完整取值语义**与"指针是否落在 UI 上"的判定 API
   （哪个查询最稳、每帧开销如何）——它决定 `PointerOverUi` 怎么报（[02](02-contract.md) 第二节）。
3. **`Dictionary` / `Array` 往返的类型映射细节**（`Variant` 与 Rust 类型的对应、
   数值溢出、`StringName` vs `String`）——它决定 [02](02-contract.md) 的字段类型怎么选。
4. **`Tween` 能否服从外部时钟**（本次只确证了 `AnimationMixer.advance()` 这条路）；
   UI 过渡若必须冻结，得先确认 `Tween` 的手动步进方式。
5. **`Resource.duplicate(true)` 在 4.5 的行为变化**（深拷贝现在只复制同文件内的资源，
   要旧行为得用 `duplicate_deep(RESOURCE_DEEP_DUPLICATE_ALL)`）——影响材质/网格的复制写法。

## 九、一帧的完整链路（示意，API 名待核对）

把前面几节串起来。左边是 GDScript 的唯一跨界点，右边是宿主那一个文件。

```gdscript
# client/scripts/sim_bridge.gd —— 只有它与宿主说话，其他脚本只读 view
extends Node

signal view_changed(view: Dictionary)

@onready var host: SimHost = %SimHost        # 🆕 gdext 类
var view: Dictionary = {}

func _ready() -> void:
    host.view_updated.connect(_on_view)      # 宿主 push，UI 不 poll（第二节纪律 2）

func _process(delta: float) -> void:
    host.step(delta)                         # 推进模拟一帧（设备事件已在 _input 里收集完）

func _unhandled_input(event: InputEvent) -> void:
    host.forward_event(event)                # 只转发 UI 没吃掉的事件（第四节）

func send_intent(name: String, payload: Dictionary = {}) -> void:
    host.send_intent(name, payload)          # 技能格点击等（02 第三节的表）

func _on_view(frame: Dictionary) -> void:
    view = frame
    view_changed.emit(frame)                 # UI 各节点接这个信号，同帧更新
```

```rust
// client/rust/src/lib.rs —— 跨界类型只有这一个（🆕，示意）
#[derive(GodotClass)]
#[class(base = Node)]
struct SimHost {
    app: App,          // MinimalPlugins + GamePlugin 的无头版本
    devices: Vec<...>, // 本帧待投递的设备事件（step 之前一次性投递）
}

#[godot_api]
impl SimHost {
    #[signal]
    fn view_updated(frame: Dictionary);

    #[func]
    fn step(&mut self, delta: f64) {
        self.flush_devices();              // 设备事件必须在 update 之前投递完
        self.app.update();                 // 世界走一帧（谁裁决全在这里面）
        let frame = view::build(self.app.world());   // 只读打包（02 第五、六节）
        self.signals().view_updated(frame);
    }

    #[func]
    fn forward_event(&mut self, event: Gd<InputEvent>) { /* → Bevy 输入消息 */ }

    #[func]
    fn send_intent(&mut self, name: GString, payload: Dictionary) { /* → 对应 Message */ }
}
```

⚠️ 两处**别写错**：

- `view::build` 里**只读**：它不许改任何游戏状态（[01](01-architecture.md) 第五节）。
- `forward_event` 只投递**设备事实**，不许在里面判断"这一下是技能还是移动"
  ——那是 `input` 域的活（[02](02-contract.md) 第一、三节）。
