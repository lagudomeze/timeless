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
      按"**别为了能看而改数据结构**"的规矩，改为新增只读**快照**
      **`PauseLabels(Vec<&'static str>)`**，由 `process_pause_requests`
      每帧**整体重建**（去重 + 排序，远程读数才可复现）；
      ④ 各 `plugin.rs` 里 `register_type`（自文档）；
      ⑤ `AGENTS.md` 的「测试规范」加一条：**诊断要读的组件 / 资源必须派生 `Reflect`**。
      **验收**：`the_diagnostic_anchors_are_reflected`（整机，遍历类型路径断言
      `ReflectComponent` / `ReflectResource` 都在；**删掉任意一个派生或那行属性都会转红**
      ——本次就是靠它发现漏了 `#[reflect(Component)]`）。
      **实机**：`world.get_resources app::clock::PauseLabels` → `["awaiting"]`、
      `app::clock::ManualPause` → `false`（以前两个都是 unknown type）。
      **为什么排在很前面**：它是**放大器**——补完之后"世界为什么冻着"直接可读，
      后面每条 UI 问题的排查成本都降一个量级（那次核查有 4 条卡在这上面）。
      ⚠️ **2026-09-27 实机复跑抓到它两个缺陷并修掉**（详见 [`CHANGELOG.md`](../../CHANGELOG.md)）：
      ① 早先的实现**只追加、从不清空**，于是它成了"这辈子出现过哪些原因"的**并集**
      ——对"此刻为什么冻着"给的是**错答案**（实测玩家在 `Executing`，镜像里却留着
      早已消失的 `awaiting`，把排查方向带偏）；现在**每帧整体重建**。
      ② 它只喂 `reasons.labels()`，而手动暂停**按设计不在** `PauseReasons` 里
      ——所以玩家自己按的那一下在诊断里**完全看不见**；现在把
      `clock::MANUAL_LABEL` 一并并进去（该常量也成了"manual"这个名字的唯一真相源，
      `presentation` 改为引用它）。
      **改名理由**：旧名 `RememberedPauseReasons` 既暗示"记住历史"、其访问器文档
      又写着"这一帧的原因表里有哪些"，名字与行为互相矛盾——现在叫 `PauseLabels`，
      装的就是"**状态行会显示的那一串**"，所以屏幕与远程读数不可能各说各话。

- [x] **#50 手动暂停的状态行漏读 `ManualPause`**（已修，**条目正文在 [`hud.md`](hud.md)**）
      放在这里只因为**根在 clock 的语义分工**：`PauseReasons` 回答"**别人**为什么停表"
      （每帧断言，下一帧不写就消失），`ManualPause` 回答"玩家自己按的暂停"
      （一次性 `Toggle`，不带原因）。
      时钟的判据是对的——`src/clock/mod.rs` 里 `paused = reasons.is_frozen() || manual.0`
      ——**漏的是显示侧**，它只看了前半个判据。
      **修法（已落地）**：HUD 在拼状态行时把 `manual` 补进原因列表；
      `PauseReasons` / `is_frozen()` **保持不动**（时钟判据本来是对的）。
      实测：`ManualPause` 现在也能被 BRP 读到（它派生进了反射表，见上面那条）。

- [x] **[P2][feat] 「冻结时视觉纪律」表**（**已落文档**）
      **问题**：冻结期间"什么该动、什么该停"只散在三处注释里，没有成文判据。
      **落点**：本条目建议的 `docs/timeline.md`（或 `domain.md`）——
      定稿在 [`docs/timeline.md`](../timeline.md) 第五节的
      **「冻结时的视觉纪律：哪类元素走哪个时钟」**，判据一句话：
      > **逻辑与"读世界状态"的视觉走 `Time<Virtual>`；
      > 只服务于"人机交互"的视觉走 `Time<Real>`。**
      表里逐行给了现值（命中粒子 / 伤害数字 / 倒计时 / 威胁格 / 悬停 / AOE 预演 →
      虚拟；提示淡出 / 日志滚动 / 面板与按钮 / 相机跟随 → 真实），
      并记了当时的统计口径：`Res<Time<Virtual>>` 39 处 vs `Res<Time<Real>>` 2 处。
      **一次真相源**：`docs/timeline.md` 是这张表的**唯一**去处，
      [`godot-client/02-contract.md`](../godot-client/02-contract.md) 与本节只**引用**它，
      不复述（同一张表写两处必然漂移——当天刚在字体字表上吃过一次这个亏）。
      **原"触发条件"已满足**：命中定帧 / 子弹时间那条现在只剩"幅度"一个决定
      （定帧改的是 `Time<Virtual>` 的流速，是这张表的下游，见
      [`presentation.md`](presentation.md)）。
