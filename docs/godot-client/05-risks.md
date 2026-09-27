# 风险与踩坑清单

> 🚧 **整篇是目标设计，代码里还没有。**
> 本篇回答一个问题：**这件事会以什么方式失败，提前怎么防。**
> 每条给「症状 → 根因 → 对策」。最后两节是**止损判据**与**上线前要亲手验的清单**。

## 一、架构类（会毁掉整件事的）

| # | 症状 | 根因 | 对策 |
| :--- | :--- | :--- | :--- |
| A1 | 两侧显示不一致，但各自单测都绿 | 同一份事实被写了两份（文案 / 键位 / 资产 / 目录表） | [02](02-contract.md) 第八节的四份对账测试；**纪律**：HUD 改动只改 `model.rs` |
| A2 | "就这一次，我在 GDScript 里算一下伤害" | 客户端越过裁决线 | 一旦越线，回放 / 联机 / 无头整机测试全部失效。**新增能力时先问"这是裁决还是画面"**：裁决加在 Rust 并发一条意图上行；画面加在 Godot |
| A3 | `src/` 里出现 `Node` / `Vector3` / `godot::` | 宿主细节渗进领域层 | **让 Cargo 依赖图强制**：模拟端 package 的依赖树里没有 `godot` crate（[01](01-architecture.md) 第三节形态乙） |
| A4 | 冻结时画面在动（或倒计时在跳） | 用 `real_delta` 补了 `world_time` 的动画 | [02](02-contract.md) 第七节的时钟判据表；这条是「时间轴显示 `RUNNING`」的翻版，**更难发现**，要专门写一条实机核查项 |
| A5 | 性能优化时把 Godot 对象塞进了视图数据 | 图快，破坏了"可序列化形状" | 视图结构体只允许 `f32` / `i32` / `String` / `Vec` / 枚举；将来拆进程时只需换 `host` |
| A6 | 每帧全量快照，CPU 看着还行但越改越慢 | 没有用 Bevy 的 `Added` / `Changed` / `RemovedComponents` 增量 | [02](02-contract.md) 第五节；现有 `TimelineCache` / `UnitPanelCache` 的 `matches()` 就是同一个思想的单机版，照它升级 |

## 二、gdext / 宿主类

| # | 症状 | 根因 | 对策 |
| :--- | :--- | :--- | :--- |
| B1 | 改一行 UI 就要重编译 Rust + 重开编辑器，迭代变慢 | 把"改得多的东西"放进了 Rust | **成本分配**：布局 / 主题 / 动画 / 文案排版 → 场景 + GDScript（改完即见）；逻辑 / 读数语义 / 目录表 → Rust（改完要重编译）。这条决定了形态乙必须是"薄宿主" |
| B2 | 出错了却不知道错在哪 | gdext 装了 panic hook：**回调里的 panic 多数被捕获**，以 `[panic …]` 形式打进 Godot 错误面板（游戏继续），但这条信息容易被当成噪声忽略 | ① 把 Godot 错误面板里的 `[panic …]` 当红灯（那是 Rust 在说话）；② 宿主再包一层 `catch_unwind`，把"出事了"变成显式状态（世界停住 + 面板提示）；③ 会 abort 的边界见 B7 |
| B3 | 换了 Godot 小版本，扩展加载失败 | gdext 版本 ↔ Godot 版本强耦合：`godot` **0.5.5** 支持 `api-4-2`…`api-4-7`（0.5 默认 **4.6**），最低 Godot **4.2**，**API 级别 ≤ 运行时版本**；未到 1.0，**minor 升级允许破坏性变更**；MSRV 是 **Rust 1.94 / edition 2024** | 版本组合登记进 [`TODO.md`](../../TODO.md) 的「依赖与文档索引」（对齐 `godot-bevy` 0.12.x ↔ Bevy 0.19 ↔ godot 0.5 ↔ Godot 4.6）；升级当作一次独立任务做（同 Bevy 升级）；注意 gdext 是 **MPL-2.0** |
| B4 | 节点操作在非主线程 → 崩或静默失败 | Godot 的 `SceneTree` 只允许主线程碰；**实例化渲染节点（`MeshInstance3D` / `Sprite3D`）默认也不是线程安全的**，多个线程改同一个资源同样不被支持 | 宿主在 `_process` 里同步 step（本方案不需要多线程）；worker 线程**只做纯数组计算、绝不碰 Godot 对象**，回主线程用 `call_deferred` 建 Mesh；官方口径是"要用线程就优先用 **Rust 线程**而不是 Godot 线程" |
| B5 | 实机排查时要在两个窗口之间来回看 | 两套日志（Rust 的 `bevy::log` 与 Godot 的 `print`） | 统一出口：Rust 日志转发到 Godot 控制台（`godot-bevy` 有现成的 `GodotBevyLogPlugin`，照抄思路）；⚠️ 已有前车之鉴：只 `use tracing_tracy::TracyLayer;` 就让扩展初始化失败（gdext issue #1230）——**日志 / trace 插件要单独验一次** |
| B6 | 玩家按暂停，UI 也跟着停了 / 世界再也解冻不了 | 用 `get_tree().paused` 或 `Engine.time_scale` 表达游戏语义 | ⚠️ **`get_tree().paused` 会把宿主自己的 `_process` 一起停掉**（`Pausable` 节点在暂停期间不再收到 `_process` / `_input`）→ 模拟不再被推进 → 世界**再也解冻不了**。**禁用**这两个 API：冻结与慢动作（含将来的"命中定帧"）都是**模拟端** `Time<Virtual>` 的事（[`docs/backlog/clock.md`](../backlog/clock.md)），客户端只读 `world_time` / `frozen` |
| B7 | 编译能过，但跑起来偶发借用 panic 或整个进程 abort | gdext 的 `Gd::bind()` / `bind_mut()` 是运行时借用检查（`bind` 后再 `base()`、类方法里 `self.to_gd()` 再 `bind_mut()` 都会立刻 panic）；**`Drop` 里的 panic 是非 unwind 的 abort** | 纪律：不要混用 `bind`/`bind_mut` 与 `base`/`base_mut`；不要在类方法里 `to_gd()` 后 `bind_mut()`；宿主入口包 `catch_unwind`（`godot-bevy` 就这么做）；**别在 `Drop` 里做可能 panic 的事** |
| B8 | 热重载后 UI 不再更新 / 偶发崩溃 | gdext 的 **typed signal** 走自带 vtable 的自定义 `Callable`，热重载后函数指针失效（官方原话：UB，通常表现为崩溃），gdext 会在重载前**自动断开**这些连接 | 连接用**标准 Callable**（`Callable::from_object_method` / `Gd::callable()`），或监听 `ObjectNotification::EXTENSION_RELOADED` 在重载后重连。⚠️ `#[class(tool)]` + 热重载官方口径是"pretty much a no go"——**别指望在编辑器里改 Rust 类还保持状态** |
| B9 | `.tscn` 被静默改坏，git diff 里才发现 | 扩展加载失败时，Godot 会**把 Rust 类静默降级成基类**并改写场景文件 | 每次改完 Rust 先编译成功再开编辑器；把 `scenes/**/*.tscn` 的意外改动当作红灯（官方明确建议用 git 检查） |
| B10 | 导出后游戏起不来（找不到扩展） | 导出时**路径必须在 `res://` 内**（开发期的 `res://../rust/target/...` 不行）；`.gdextension` 指向的 `.dll` 还要与 profile 匹配 | 导出前把动态库拷进项目目录（一条脚本）；debug / release 不许混；导出步骤也归 E4 |

## 三、Godot 侧渲染类

| # | 症状 | 根因 | 对策 |
| :--- | :--- | :--- | :--- |
| C1 | 地形一片纯色（AO 与面朝向的明暗全没了） | 明暗烘焙在**顶点色**里（`face_shade` × AO），材质没用它 | 材质必须把顶点色当 albedo 用（Godot 4：`BaseMaterial3D.vertex_color_use_as_albedo = true`）。⚠️ 症状像"网格坏了"，实际是材质设置 |
| C2 | **整片地形看不见 / 从下面才看得见**（顶点数、面积、AO 全对） | **绕序方向相反**：Godot 的三角形正面是**顺时针**，而 Rust 侧按"从外侧逆时针"生成（有测试 `every_greedy_quad_winds_counter_clockwise_from_outside` 钉着）→ 全部被背面剔除 | 上传时**反转每个三角形的索引顺序**（推荐，数据形状与 Rust 测试都不动）；或材质 `cull_mode = CULL_DISABLED`；或在网格化阶段翻转并改那条测试。**P1 第一件事**：一个孤立方块六面可见，再上整片地形 |
| C3 | 贴图被拉伸铺满整片地面 / 糊成一团 | 贪婪合并的 UV 跨度是**格数**（最大 = `CHUNK_SIZE`，见 `merged_faces_tile_their_texture_by_the_rectangle_size`），材质没开重复；过滤设错了作用域 | 材质纹理 repeat；**过滤的作用域要分清**：`rendering/textures/canvas_textures/*` 只管 2D，3D 要逐材质（`BaseMaterial3D.texture_filter`）或逐节点（`SpriteBase3D.texture_filter`，默认带 mipmap 的线性过滤）设 nearest |
| C4 | 地面指示（悬停 / 威胁 / AOE）闪烁、与纸片单位互相穿插 | 半透明贴片与地形共面 → 深度冲突；而透明物体是"整个对象"参与从后往前排序，一整块大平面会被当成一个点 | **z-fighting 的正解是几何抬升**（沿世界 Y 抬 0.01~0.05）或独立渲染层；`render_priority` **只排透明物体之间的先后，不解除深度冲突**。高亮**拆成每格一个小面**更稳；要柔边又稳就用 `ALPHA_CUT_OPAQUE_PREPASS`（较慢），懒得管柔边就用 `ALPHA_CUT_DISCARD` |
| C5 | 纸片单位朝向不对 / 贴地阴影在某些机器上不显示 | `billboard` **默认是 Disabled**，正交相机不会自动让它面向相机；阴影若用 `Decal`，则 Compatibility 渲染方式**完全不支持**、Mobile 下每个 mesh 最多 8 个、过滤是**全局**项目设置 | 显式设 `BaseMaterial3D.BILLBOARD_FIXED_Y`（**不要** `BILLBOARD_ENABLED`，斜视角下纸片会翻倒）；阴影优先用"略微抬高的透明 quad"，只有必须贴合台阶地形时才用 `Decal` + `cull_mask` 隔离 |
| C6 | 每次挖一块方块会卡一下 | 区块网格重建 + `ArrayMesh` 上传都在主线程 | 网格**计算**仍在 Rust 的异步任务池（现在就是），客户端只做"接收数组 → 建 `ArrayMesh`"；⚠️ **实例化渲染节点默认不是线程安全的**，所以 worker 只算纯数组、回主线程用 `call_deferred` 建 Mesh；必要时合并同帧的多个区块重建 |
| C7 | 装饰物（树 / 石头）数量一多就掉帧 | 每个 `.glb` 实例一个节点 | 用 `MultiMeshInstance3D` 或分批实例化。⚠️ MultiMesh 有几条硬规矩：`mesh` 必须是 **Mesh 资源**（`Sprite3D` 要先 `generate_triangle_mesh()` 转）、`use_colors` / `use_custom_data` **必须在 `instance_count` 之前设**、AABB 要自己给（`custom_aabb`），且几十个实例时**没必要**用它 |

## 四、UI 类

| # | 症状 | 根因 | 对策 |
| :--- | :--- | :--- | :--- |
| D1 | 中文变成方块 / 小字号的中文糊成一团 | 前者是 Godot 默认字体不含 CJK（与 Bevy 同一个坑）；后者往往是**给中文正文用了 MSDF**——它对"密集细笔画 + 小字号"容易糊或缺笔画，而中文正好笔画密 | 主题里显式配 `assets/fonts/NotoSansSC-Regular.otf`：**中文正文用动态字体**，MSDF 只留给需要大幅缩放的标题，并配 `FontFile.fallbacks` 做缺字回退。`tests/assets.rs` 的**逐字 `cmap` 验收继续有效**（文案仍由 Rust 生成），前提是两侧**共用同一份字体文件** |
| D2 | 高分屏下面板过大 / 过小，或双重缩放 | 现在有 `fit_ui_scale_system`（按窗口高度把 `UiScale` 夹在 `MIN_UI_SCALE..MAX_UI_SCALE`），而 **Godot 没有"按高度等比缩放整套 UI"的单一开关**——只有 stretch mode / aspect / `content_scale_factor` 的组合 | 三选一（[03](03-client.md) 第五节）：`canvas_items + keep_height`、`canvas_items + keep_width`、或 `mode = disabled` 自己按 `window.size.y / BASE_HEIGHT` 缩放根节点。**只保留一套机制**；`BASE_HEIGHT` / 两个上下限从 `Directory.constants` 传，别在两处各写一个数；像素风另加 `scale_mode = integer` |
| D3 | 数字对不齐，有人往数据里补空格 | 老坑（[`docs/backlog/hud.md`](../backlog/hud.md) #51：`cell ( 1, 0)`） | 对位交给字体或容器（`HBoxContainer` + 固定宽度），**不许把空格写进 Rust 生成的文案** |
| D4 | 搬家把老 bug 一起搬过去（敌人面板压住日志） | 直接照着现有布局复刻 | **分两轮**：先"逐字符对齐地搬"（此时对账测试才分得清"没搬对"与"改过了"），搬完再单独一轮改布局 |
| D5 | 点面板顺手把角色走了一格 | `PointerOverUi` 这条事实在新机制下丢了 | Godot 报 `PointerOverUi`；[`playtest-checklist.md`](../playtest-checklist.md) 第 3 节那条"每个面板上都点一下"必须有等价验收 |
| D6 | 时间轴色块与倒计时在冻结时抖动 | 用真实帧间隔推进了世界时基 | 见 A4：色块 / 刻线 / 倒计时一律 `world_time` |

## 五、工程与流程类

| # | 症状 | 根因 | 对策 |
| :--- | :--- | :--- | :--- |
| E1 | 资产变成两份，改一份另一份不知道 | Godot 的 `res://` 必须在项目目录内，而资产在仓库根的 `assets/`；**导出后 `res://` 在 PCK 内只读** | 二选一：① Godot 项目建在仓库根（`res://assets` 就是 `assets/`）；② 项目建在 `client/`，用目录联接 / 拷贝脚本把 `assets/` 暴露成 `res://assets`——**导出前资产必须真的在项目目录内**。**提交策略**：`*.import` **要提交**（记录导入参数与 uid），`.godot/`（导入缓存 + 编辑器状态）**不要提交**。再加一条路径对账测试（两侧看到同一批文件） |
| E2 | 改一行 Rust 要等很久（Bevy 被编译两遍） | `client/rust/` 是独立 package，默认有自己的 `target/` 与 `Cargo.lock`，与根 package 各编一份 | 两个选择：① **默认各自独立**（不抢 target 锁，代价是磁盘与编译时间翻倍）；② 共享 `CARGO_TARGET_DIR`（省编译，但根 package 与 `client/` 的两个 cargo 调用会**抢同一把 target 锁**——"`cargo run` 与 `cargo test` 抢锁"那条坑的加强版）。⚠️ 无论哪种，**两个 `Cargo.lock` 里的 bevy 版本必须同步升级**，否则同一份 `app` 源码会被编进两个不同的 bevy 版本 |
| E3 | 实机核查成本翻倍（要起两个进程） | 现在是"跑 `cargo run`"一步 | 一条脚本一键起两侧；实机清单加"两侧都在同一状态"的前置检查（比如都显示同一个 `world_time`） |
| E4 | 导出包缺动态库 / 版本不匹配 | `.gdextension` 指向的 `.dll` 路径与构建 profile 不一致 | 导出流程写进文档；debug / release 动态库不许混用 |
| E5 | Godot 侧逻辑没有测试，只能靠手点 | 认为"UI 不用测" | Godot 侧至少要有**三类测试**：场景契约（节点名与绑定齐全）、离线快照渲染（喂 `--dump-view` 的文件，断言读数文本）、以及"意图发送器全被引用"（[02](02-contract.md) 第八节） |
| E6 | CI 上 Godot 那条命令时灵时不灵 | 无头运行的几条硬约束：① 无 GPU 的机器**必须 `--headless`**（且 headless 下 `Engine.get_frames_drawn()` **恒为 0**，"像素断言"做不到）；② **首次必须 `--import`** 让资源导入；③ 导出要求 preset 名与 `export_presets.cfg` 完全一致、**目标目录必须预先存在**；④ 相对路径是相对 `project.godot` 而不是 cwd；⑤ 编辑器版本必须与导出模板版本一致 | 把 CI 命令写成脚本并**固定版本**；像素级对比只在有 GPU 的机器上跑（`--write-movie` + `--fixed-fps`），CI 里只断言"渲染输入"（顶点/索引数组、材质属性、Control rect、读数文本）；第三方测试框架（GUT / GdUnit4）的命令**先查官方文档再进 CI** |

## 六、止损判据（什么时候承认这条路走错了）

| 观察到的现象 | 说明什么 | 怎么处理 |
| :--- | :--- | :--- |
| P1 做完发现每帧跨边界的开销无法接受 | 形态乙的宿主边界太宽 | 先收窄契约（只传变化），不要立刻上 IPC；仍不行则回退"Bevy 继续渲染、只搬 HUD" |
| P2 搬到第二块时发现每块要重写 40% 的语义 | `model.rs` 没切干净（语义还混在 `system.rs` 里） | **停下来先把 model 层切干净**再继续，别硬推——否则两侧会长期分叉 |
| 日常迭代被"重编译 + 重开编辑器"拖慢 | 改变频率高的东西放错了层 | 把更多东西往场景 / GDScript 推（B1）；必要时直接上形态丙（IPC + GDScript 客户端），让 Rust 只在需要时才重建 |
| 客户端越来越"懂"游戏规则 | 裁决线在慢慢溶掉 | 立刻回头审 A2：任何一处"客户端算出来的结果"都要改成"模拟端给的读数" |

## 七、上线前要亲手验的清单（合并进 review 流程）

1. **六面可见**：一个孤立方块在 Godot 里六个面都能看见（验绕序 C2）。
2. **明暗在**：墙根的角落比敞开的角暗（验顶点色 C1）。
3. **贴图平铺**：一整片地面的方块纹理不被拉伸（验 UV C3）。
4. **冻结纪律**：玩家空闲时，色块 / 倒计时 / 粒子全部停住，**面板与提示条仍可交互**（验 A4）。
5. **三种冻结原因**：`awaiting` / `threat` / `manual` 都显示正确，叠加时按字母序拼（含 #50）。
6. **读数逐字符一致**：时间轴状态行、面板状态行、日志行与 Bevy 版逐字符相同（P2/P3 期间每一块都要过）。
7. **UI 穿透**：每个面板上点一下，世界不动、悬停高亮与预演消失。
8. **一键重置**：`F5` / 重开按钮之后两侧状态一致（无残留节点、无幽灵实体——`ViewId` 的回收登记要靠它验）。
9. **BRP 仍可用**：`world.query` 读得到决策槽与 `PauseReasons`（无头之后它是唯一的"透视眼"）。
10. **无显卡可跑**：在没有 GPU 的环境里 `cargo test` 全绿（这是无头化最大的副产品，别浪费）。
