# backlog · clock（冻结设施与诊断锚点）

> 活的总览：[`../../TODO.md`](../../TODO.md) · 历史证据：[`../../CHANGELOG.md`](../../CHANGELOG.md)
> ⚠️ 这是**通用冻结设施**所在的域，不属于任何玩法领域——改它之前先读
> [`docs/domain.md`](../domain.md) 与 `src/clock/mod.rs` 的文首（两种暂停时序别混）。

- [x] **#62 诊断锚点补 Reflect：BRP 读不到「世界为什么冻着」**（已修）
      **现象（那次核查的真实摩擦）**：用 BRP 远程核查时最想问的两件事都读不到——
      「世界为什么冻着」与「反应窗口表态了没有」：
      ```text
      world.get_resources app::clock::PauseReasons
      → Unknown resource type (error -23501)
      ```
      同样读不到：`ManualPause` / `ReactionSlot` / `Threatens` / `TargetCell` /
      `MenuSelection` / `UnitSprites` / `BattleLog`。核查只能从状态行**文字**倒推，
      而文字本身正是被核查的对象——**循环论证**。
      **改法（已落地）**：
      ① 组件补 `Reflect` + **`#[reflect(Component)]`**（只有 `derive` 还不够：
      查询要的 `ReflectComponent` 来自那行属性——踩过，测试当场报
      "没作为组件注册"）：`ReactionSlot` / `Threatens` / `TargetCell` / `Threatened` /
      `MainCamera`；
      ② 资源补 `Reflect` + `#[reflect(Resource)]`：`ManualPause` / `MenuSelection` /
      `BattleLog` / `UnitSprites`；`CounterSuggestion` 与它嵌的 `CounterCost` 一起派生；
      ③ **`PauseReasons` 不派生**（内含 `HashSet<&'static str>`，反射要额外支持）——
      按"**别为了能看而改数据结构**"的规矩，改为新增只读镜像
      **`RememberedPauseReasons(Vec<&'static str>)`**，由 `process_pause_requests`
      每帧顺手记（去重 + 排序，远程读数才可复现）；
      ④ 各 `plugin.rs` 里 `register_type`（自文档）；
      ⑤ `AGENTS.md` 的「测试规范」加一条：**诊断要读的组件 / 资源必须派生 `Reflect`**。
      **验收**：`the_diagnostic_anchors_are_reflected`（整机，遍历类型路径断言
      `ReflectComponent` / `ReflectResource` 都在；**删掉任意一个派生或那行属性都会转红**
      ——本次就是靠它发现漏了 `#[reflect(Component)]`）。
      **实机**：`world.get_resources app::clock::RememberedPauseReasons` → `["awaiting"]`、
      `app::clock::ManualPause` → `false`（以前两个都是 unknown type）。
      **为什么排在很前面**：它是**放大器**——补完之后"世界为什么冻着"直接可读，
      后面每条 UI 问题的排查成本都降一个量级（那次核查有 4 条卡在这上面）。

- [x] **#50 手动暂停的状态行漏读 `ManualPause`**（已修，**条目正文在 [`hud.md`](hud.md)**）
      放在这里只因为**根在 clock 的语义分工**：`PauseReasons` 回答"**别人**为什么停表"
      （每帧断言，下一帧不写就消失），`ManualPause` 回答"玩家自己按的暂停"
      （一次性 `Toggle`，不带原因）。
      时钟的判据是对的——`src/clock/mod.rs` 里 `paused = reasons.is_frozen() || manual.0`
      ——**漏的是显示侧**，它只看了前半个判据。
      **修法（已落地）**：HUD 在拼状态行时把 `manual` 补进原因列表；
      `PauseReasons` / `is_frozen()` **保持不动**（时钟判据本来是对的）。
      实测：`ManualPause` 现在也能被 BRP 读到（它派生进了反射表，见上面那条）。

- [ ] **[P2][feat] 「冻结时视觉纪律」表**（本次核查提出，**需要拍板后落文档**）
      **问题**：冻结期间"什么该动、什么该停"目前只散在三处注释里，没有成文判据：
      | 元素 | 现在用的时钟 | 为什么 |
      | :--- | :--- | :--- |
      | 提示条淡出 | `Time<Real>`（`hud/hint.rs`） | 世界冻着提示也得能自己消失 |
      | 相机跟随 | `Time<Real>`（`presentation/camera.rs`） | 冻结时把上一段移动追完，不僵在半路 |
      | 命中粒子 | `Time<Virtual>`（`presentation/effects.rs`） | 冻结时**定格在"刚打中"**，玩家解冻后看完 |
      | 时间轴读数倒计时 | `Time<Virtual>` | 倒计时要与世界同步，冻结时不该继续跳 |
      **改法**：把上表写进 `docs/domain.md`（或 `docs/timeline.md`）并补一条判据：
      > **逻辑与"读世界状态"的视觉走 `Time<Virtual>`；只服务于"人机交互"的视觉走 `Time<Real>`。**
      **触发条件**：在加"命中定帧 / 子弹时间"（[`presentation.md`](presentation.md) 那条）
      之前**必须先定这张表**——定帧本身就是改 `Time<Virtual>` 的流速，没有判据会变成每加一个
      特效讨论一次。
      注：核查建议的"子弹时间"（`relative_speed` 降到 0.1）可以作为**这张表上的一行**，
      不需要新机制。
