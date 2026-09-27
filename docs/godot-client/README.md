# 逻辑与表现分家：Bevy 只跑游戏逻辑，Godot 做表现与交互

> 🚧 **整篇是目标设计（design only），代码一行都没落地。**
> 本篇是这一组文档的**入口**：先给结论，再给方案对比与阅读顺序。
> 维护规则同 [`docs/index.md`](../index.md)：一篇只回答一个问题，
> 引用的标识符必须能在 `src/` 里 grep 到（本文出现的**新**名字一律标 🆕）。

## 一、结论

**可以做，而且本项目比一般项目更划算——但它是一次「表现层重写」，不是"换一个 UI 框架"。**

划算的地方在于本项目**已经具备**两个前提：

1. **逻辑整机可以无头运行**——`crate::test_support::headless_app()`
   （[`src/lib.rs`](../../src/lib.rs)）已经是"除渲染外的整机 App"，
   `src/lib.rs` 里几十条整机用例跑的就是它。这次改造本质上是**把测试夹具变成产品入口**。
2. **世界的大部分时间是冻结的**（等玩家决策 / 威胁逼近时 `Time<Virtual>` 停表）——
   进程边界带来的延迟被这个设计天然吃掉，甚至将来联机也不需要客户端预测。

要付的账：`presentation`（相机 / 纸片 / 装饰 / 特效 / 威胁格 / HUD）
+ `interaction::visual`（高亮与预演）+ `voxel_render` 的材质与网格资产层
+ `spawn` 里的视觉子节点组装，全部要重写；**约 58 / 167 个源文件会被碰到**。
好消息是表现层里最值钱的那一半（读数语义、文案、拾取、网格化）是**带走**的，
不是重写的（明细见 [04-migration.md](04-migration.md) 第七节）。

顺带拿到的副产品：**无 GPU 也能跑完整逻辑测试**（CI 友好）、
视图快照可回放、以及将来联机是"换一层传输"而不是重写。

## 二、目标形态（一句话）

```text
Bevy（无头，唯一权威）   ←─ 设备事件 + 姿态 ──  Godot 4（唯一窗口，只画与只转发）
  领域：world / movement / combat / skills /     世界：地形 / 纸片单位 / 地面指示 / 特效
  equipment / timeline / clock / ai / input       UI：时间轴 / 面板 / 技能栏 / 日志 / 帮助
  + 🆕 view（视图导出）+ 🆕 host（宿主薄壳）      ←─ 数据视图（读数 + 世界 + 时钟头）─
```

- **客户端永不裁决**：不改血、不掷骰、不算伤害、不判可行走性。
- **模拟端永不认识 Godot**：领域层零 `godot` 类型（由 Cargo 依赖图强制）。
- **HUD 的"语义"留在 Rust，"排版与绘制"去 Godot**：
  这样 [`docs/backlog/hud.md`](../backlog/hud.md) 里那批「界面在说谎」的 bug
  所对应的纯函数测试**不会被丢掉**。

## 三、你问的那个问题：gdext 和 GDScript 有什么异同？对方案有什么参考意义？

先说结论：**在这个方案里它们不是二选一，而是天然的分工——gdext 做"宿主"（很薄），
GDScript 做"表现与胶水"（很厚）。**

### 3.1 先摆事实（已查证，2026-09）

| 项 | 事实 | 来源 |
| :--- | :--- | :--- |
| `godot` crate（gdext） | 最新 **0.5.5**；支持 API 级别 `api-4-2` … `api-4-7`（0.5 默认 **4.6**）；最低 Godot 运行时 **4.2**；MSRV **Rust 1.94 / edition 2024**；许可 MPL-2.0 | [gdext releases](https://github.com/godot-rust/gdext/releases)、[版本选择](https://godot-rust.github.io/book/toolchain/godot-version.html) |
| 与 Godot 的关系 | **API 级别 ≤ 运行时版本**；GDExtension 向后兼容（4.2 的扩展能跑在 4.3 上，反过来不行）；未到 1.0，**minor 升级允许破坏性变更** | 同上 |
| 热的"热重载" | Godot **4.2 起**才允许"编辑器开着时重编译"；`reloadable = true` 的确切语义是"**编辑器窗口失焦/重新获得焦点时**重载"；官方 tracker 里仍有一串未清的热重载问题（尤其 `#[class(tool)]` 节点热重载后**丢状态、`ready` 不再执行**） | [Hello World 章](https://godot-rust.github.io/book/intro/hello-world.html)、[tracker #1221](https://github.com/godot-rust/gdext/issues/1221) |
| 编辑器怎么用 Rust 类 | **不是"挂脚本"，是"Change Type…"**：场景里右键节点改成你的 Rust 类；⚠️ 若扩展加载失败，Godot 会**静默把它降级成基类**并改写 `.tscn`——要靠 git diff 才发现 | [Hello World 章](https://godot-rust.github.io/book/intro/hello-world.html) |
| 信号与热重载 | gdext 的**typed signal** 走自带 vtable 的自定义 `Callable`，**热重载后这些函数指针失效（UB/崩溃）**；gdext 的做法是热重载前自动断开全部 typed 连接，之后用 `EXTENSION_RELOADED` 重建。**标准 Callable**（`Callable::from_object_method`）才能活过热重载 | gdext 文档（Signals 章） |
| 已有先例 | **`bytemeadow/godot-bevy`（555★，0.12.0）**：把 Bevy `App` 作为 Godot 的 `Node`（autoload）跑在 GDExtension 里，**版本矩阵与本项目逐项对齐：godot-bevy 0.12.x ↔ Bevy 0.19 ↔ godot-rust 0.5 ↔ Godot 4.6** | [godot-bevy](https://github.com/bytemeadow/godot-bevy) |
| 同进程的代价 | Godot API **只能在主线程**碰（SceneTree 尤其）；gdext 的 `experimental-threads` feature 官方自述"**high risk of unsoundness**"。`godot-bevy` 的默认 feature 里**开着**它 | gdext 文档、godot-bevy 仓库 |
| panic 边界 | gdext 装了 panic hook，回调里的 panic 多数被**捕获并报进 Godot 错误面板**（游戏继续）；但 **`Drop` 里的 panic 是非 unwind 的 abort**，double borrow 也走 abort → 会**连编辑器一起带走** | gdext issue（#1668 / #1427 等） |
| 性能口径 | 官方只有定性的"**GDExtension（C++）几乎总是比 C# 与 GDScript 快**"，**没有**具体倍数；gdext 另有 strict / balanced / disengaged 三档安全检查（默认 dev=strict、release=balanced） | Godot FAQ、gdext 文档 |
| gdext 的自我定位 | 它是**纯 Godot 绑定**；"与其它生态（**ECS**、资产管线、GUI）的集成 **out of scope**，应由扩展实现"——所以 A1 这条路**永远得靠第三方桥或自己写** | godot-rust book（Philosophy 章） |
| 没有先例的那条路 | 公开项目里 **A1（同进程）一边倒**；**没有**找到"Bevy 独立无头进程 + 本地 IPC 到 Godot"的公开项目或文章（[01](01-architecture.md) 的形态丙要自己承担协议设计） | GitHub 搜索（"bevy godot" 按 star 排序） |

> ⚠️ 以上是"以官方文档 / 仓库 / crates 元数据为准"的查证结果；`web_search` 后端在本机不可用，
> 所以**论坛/社区经验帖没有覆盖**（可能存在的 Reddit / Discord 一手经验不在表内）。

### 3.2 三者对比

| 维度 | GDScript | gdext（Rust 绑定） | C# |
| :--- | :--- | :--- | :--- |
| 迭代速度 | 最快：保存即生效，不用编译 | 最慢：改 Rust 要重编译动态库（4.2+ 可热重载，但见 3.1 那张表） | 中间 |
| 编辑器集成 | 一等公民：任意节点挂脚本、`@export` 变量、信号在编辑器里连线 | 注册的类能当节点 / 资源类型用（**Change Type** 的路子）；"给任意节点挂脚本"这类工作流支持得较弱 | 与 GDScript 接近 |
| 类型与生态 | 动态类型 + 编辑器静态检查；**signal 参数不做类型检查** | 强类型（**typed signal 编译期校验**）+ 整个 Rust 生态；**能直接 `use` 根 package 的逻辑类型** | 强类型 + .NET 生态 |
| 性能 | 与 C# 同数量级（官方口径） | 与 C++/GDExtension 同级、"几乎总是更快"（**无具体倍数**） | 同数量级 |
| 与本项目逻辑的关系 | 必须跨边界传数据 | **可以同进程直接读 Bevy `World`** | 必须跨边界传数据 |
| 调试体验 | 编辑器里断点 / 单步最好 | 用 **LLDB attach 到 Godot 进程**；⚠️ **编辑器模式下断点不保留**，实际工作流是从命令行/VS Code 起游戏再 attach；只有 Rust 帧有符号 | 较好 |
| 部署 | 无额外产物 | 要带动态库 + `.gdextension`；⚠️ **导出时 dylib 路径必须在 `res://` 内**（开发期的 `res://../rust/target/...` 不行），要拷进工程 | 要 .NET 运行时 |
| 测试 | GUT / GdUnit4 | 有 Rust 侧的 Godot 集成测试框架 | 常用 .NET 测试框架 |

### 3.3 gdext 对本方案的真正价值（以及一个现成的先例）

**不是"用 Rust 写 UI 更爽"**（并不爽），而是：

> **gdext 让你不必发明一套 IPC 协议。** 宿主与逻辑同在一个进程里，
> 视图数据可以**直接读 `World`**，设备事件可以**直接写成 Bevy 的输入消息**。

这就是 [01-architecture.md](01-architecture.md) 里"形态乙"的全部要点：
Rust 侧只暴露一个节点（🆕 `SimHost`），对外是 `step()` / `send_intent()` /
`send_key()` / 一个 `view_updated(frame)` 信号；跨边界的类型是
`Dictionary` / `Array` / 数值 / 字符串——**也就是 [02-contract.md](02-contract.md)
要求的"可序列化形状"**。将来真要拆进程（联机 / 专用服务器），换掉 `SimHost`
的实现即可，右侧那些 `.tscn` 与 GDScript 一行不用动。

**先例告诉我们两件重要的事**（`godot-bevy`，见 3.1）：

1. **"Bevy App 跑在 Godot 的 Node 里"是可行的、有维护中的实现，而且版本栈与本项目对齐。**
   所以 P−1 的 spike（[04](04-migration.md)）**不必从零试**：先读它怎么处理
   panic 捕获、主线程边界与日志桥接，能抄就抄。
2. **但它的设计哲学与本方案相反**：它明确说自己是"给 Godot 开发者用 Rust/ECS"，
   理念是 *Godot for Content, Bevy for Logic*，并且会把**整棵 Godot 场景树镜像成 ECS 实体**；
   生产驱动方式是**拆 Bevy 的 `Main`**（前半在 `_physics_process`、后半在 `_process`），
   文档里写明"**生产环境从不调用 `app.update()`**"。
   本方案要的是"**Bevy 是唯一权威、Godot 节点只是被动视图**"，
   所以**只借它的机制，不用它的场景镜像**；至于"一次 `_process` = 一次 `app.update()`"
   这条更贴合本项目冻结语义的驱动方式，**要在 spike 里验**（它的文档只承认手动
   `update()` 合法，但"不要与生产驱动混用"）。

### 3.4 所以怎么用

| 选择 | 结果 |
| :--- | :--- |
| **全部用 gdext 写（含 UI 胶水）** | 省了协议，但 UI 迭代要重编译（还要跟热重载的坑打交道）——而 UI 恰恰是改得最频繁的部分。**不推荐** |
| **全部用 GDScript（Bevy 独立进程 + IPC）** | UI 最舒服，但你必须先写完协议、序列化、连接生命周期才能看见第一个面板；而且**没有公开先例可抄**（3.1 最后一行）。**留作退路**（[01](01-architecture.md) 形态丙） |
| **gdext 宿主 + GDScript 表现** ⭐ | 零协议 + 编辑器工作流，两边都拿到。宿主可以**先评估 godot-bevy**，不行再自己写薄壳。**推荐**（形态乙） |
| **用 gdext 但只碰主线程**（本方案的纪律） | ⚠️ 这条让我们**有机会不打开 `experimental-threads`**（官方自述"high risk of unsoundness"）：所有 Godot 触碰集中在 `SimHost` 一个文件、跑在主线程；网格化等重计算留在 Bevy 任务池，但**只传出纯数组，不传 Godot 对象**。**能不能不开，要在 spike 里验** |

一句话：**改变频率低的东西写 Rust（逻辑 + 读数语义），改变频率高的东西写场景/GDScript
（布局 + 主题 + 动画 + 胶水）。** 这条成本分配是形态乙成立的关键。

## 四、先确认痛在哪（三条更便宜的路线，先量一下再决定）

这次改造的成本是"重写表现层"。所以值得先花半天量一下：最近的 UI / 交互工作
到底卡在哪一步？

| 如果痛的是…… | 症状 | 更便宜的解法（相对本次改造） |
| :--- | :--- | :--- |
| **样板代码多** | 拿数 → 算文案 → 写 `Text`/`Node` 三处重复；每加一个读数要改四个文件 | 在 Bevy 里加一层**声明式绑定**（一个 `Widget`/`bind!` 抽象 + 一个通用同步系统），把"取数 → 比对 → 写 UI"收成一处。成本约是本次改造的 5% |
| **没有可视化编辑器** | 布局靠手算像素与百分比锚点；改一次间距要重编译；`fit_ui_scale_system` 这类适配逻辑得自己写 | **Bevy 生态里没有等价物**——这就是本次改造最硬的理由（Godot 的 `.tscn` + 容器 + 主题 + 动画正是你缺的东西） |
| **迭代慢** | 改一句文案都要等 `cargo build` | 现成的便宜路线：`hot-reload` feature、把 UI 数据/布局外置成 `ron`/`json`、把 HUD 拆成更小的 crate。**别为了这个换引擎** |

> 我的判断：你的诉求（"交互和 UI 实现比较累"）里，**第 2 条占比最大**，
> 所以这次改造方向是对的；但请把它当成一个**有阶段验收的迁移项目**
> （[04-migration.md](04-migration.md)），而不是"接一个库"。

## 五、文档地图与阅读顺序

| 篇 | 回答什么 | 什么时候读 |
| :--- | :--- | :--- |
| [01-architecture.md](01-architecture.md) | **边界画在哪**：谁裁决什么、三种进程形态、每个域的去留 | 决定要不要做、以及先跟人吵清楚"哪些东西不搬" |
| [02-contract.md](02-contract.md) | **两侧怎么说话**：设备事件 / 姿态 / UI 意图上行，静态目录 / 世界视图 / 读数视图 / 时钟下行，以及对账测试 | 写第一行代码之前 |
| [03-client.md](03-client.md) | **Godot 那一侧长什么样**：目录、`SimHost`、地形与单位渲染、输入与焦点、HUD 逐块对应 | 建 Godot 项目时 |
| [04-migration.md](04-migration.md) | **怎么一步步搬**：P0–P4 五段，每段的动作与退出标准 | 排期与每个阶段开工时 |
| [05-risks.md](05-risks.md) | **会怎么失败**：风险表、止损判据、上线前要亲手验的 10 条 | 每次开工前扫一眼 |

建议顺序：**01 → 02 →（拍板）→ 04 的 P0 → 03 → 04 的 P1…**
⚠️ 别从 03 开始：先把边界与契约定死，否则 Godot 侧会顺手把规则抄进去，
那是最贵的返工。

## 六、需要你拍板的六件事（文档里都给了建议）

| # | 决定 | 文档给的建议 |
| :--- | :--- | :--- |
| 1 | 客户端胶水用 GDScript 还是全 Rust | **GDScript 厚、gdext 薄**（本篇第三节） |
| 2 | 相机归谁 | **归客户端**，`presentation::camera` 随之删除（[01](01-architecture.md) 第三节） |
| 3 | `PanCamera` / `ZoomCamera` 是否还上行 | 同上，**不上行** |
| 4 | `voxel_render` 改造后叫什么 | 改名 🆕 `meshing`（保留独立域，[04](04-migration.md) P4） |
| 5 | `F1`（帮助面板）算游戏键还是 UI 键 | **算 UI 键**，`HELP_LINES` 随静态目录下发（[03](03-client.md) 第四节） |
| 6 | `assets/` 怎么给 Godot 用 | 目录联接暴露 `res://assets` + 一条路径对账测试（[05](05-risks.md) E1） |
| 7 | 宿主用 `godot-bevy` 还是自己写 | **先评估 `godot-bevy`**（省掉一大块），但它要满足两个条件：场景树镜像能退场、驱动方式能改成"一次 `_process` = 一次 `app.update()`"；不满足就自己写薄壳（约 200 行），panic 兜底与日志桥照抄它（[01](01-architecture.md) 第三节） |

## 七、这份设计与仓库既有约定的关系

- 它**不推翻**任何铁律，而是让其中两条更硬：
  「表现层只读」升级为**「客户端永不裁决」**，「领域层零 Bevy」旁边新增**「领域层零 Godot」**
  （[01](01-architecture.md) 第六节）。
- 它**复用**已有的资产：`src/lib.rs` 的 `headless_app()`、`input/keyboard.rs` 的键位真相、
  `hud/*/model.rs` 的读数语义与那批单测、`tests/assets.rs` 的字体 / 图标验收、
  `docs/playtest-checklist.md` 的实机清单（只需把截图手段换掉）。
- 它是**目标设计**，所以按 [`docs/index.md`](../index.md) 的规矩：本文档组里
  出现的新类型名一律标 🆕，落地多少就在 [`TODO.md`](../../TODO.md) 里勾多少。
