# backlog · HUD（面板 / 时间轴 / 技能栏 / 帮助 / 日志）

> 活的总览：[`../../TODO.md`](../../TODO.md) · 历史证据：[`../../CHANGELOG.md`](../../CHANGELOG.md)

- [x] **#47 `ready` / `busy` 判据反了**（已修）
      **根因**：`panels/model.rs` 的 `defense_label()` 用了 `DecisionSlot::ready()`
      （语义是"**已经决定了** / 别等他"），而面板要的是 `is_idle()`（"现在能决策"）。
      两个 `ready` 撞名，读反了。
      **改法**：① 判据换 `is_idle()`；② **消除撞名**——`DecisionSlot::ready()` 改名
      **`decided()`**，调用点全部更新（`panels/model.rs`、`lib.rs` 整机用例、
      `timeline/wait.rs`、`clock/mod.rs` 的注释、`decision.rs` 的文档与两条用例名）。
      **验收**：`an_idle_unit_reads_ready_and_a_busy_one_reads_busy`（模型层，
      **三个方向**：空闲 → `ready`、`Executing` → `busy`、`Idle { intent: Some }` → `busy`）；
      `a_fresh_slot_is_idle_and_not_decided` / `the_two_predicates_answer_different_questions`
      改名后仍绿。
      **实机**：`PLAYER · ready · cell (1,0)` 与 `ENEMY 1 · busy · cell (3,3)`
      ——正好与决策槽（`Idle { intent: null }` / `Executing`）对上。

- [x] **#52 Focus 没有 HUD 读数**（已修）
      **改法**：① `UnitRow` 加 `focus: Option<Focus>`；② `UnitPanels::focus_text`
      （`FOCUS 2 / 3`，缺组件写 `FOCUS -`）与 `focus_pips`（三点式 `[bool; FOCUS_PIPS]`）；
      ③ `panels/scene.rs` 在 EN 条下面加一行「`FOCUS n / 3` + 三个圆点」，
      **玩家与敌人都画**（敌人也会花 Focus 闪避，对称才看得见"它刚买掉了前摇"）；
      ④ `panels/system.rs` 查询补 `Option<&Focus>`，圆点只改颜色（池化，帧内不建实体）；
      ⑤ `layout.rs` 的具名节点表补 `*FocusLine` / `*FocusText` / `*Focus0`。
      **验收**：`focus_pips_mark_the_spent_points`（满 / 用 1 点 / 无组件三个方向）、
      `the_pip_count_matches_the_focus_ceiling`（圆点数跟住 `FOCUS_MAX`）、
      `layout.rs` 具名节点清单。
      **实机**：`FOCUS 3 / 3` + 三个亮点（玩家与两个敌人各一行）。

- [x] **#51 `cell ( 1, 0)` 多一个前导空格**（已修）
      去掉 `{:>2}`（对齐交给字体），并改掉那个**把错误格式钉死**的断言
      （`state_line_carries_defense_and_intent` 里的 `"cell ( 3, 3)"`）。
      **实机**：`cell (1,0)` / `cell (3,3)`。

- [x] **#54 帮助面板没教核心循环**（已修）
      `HELP_LINES` 最前面加一栏 `CORE LOOP`（世界只为谁冻结 / 时间轴色块怎么读 /
      威胁的两条出路 / Focus 怎么花 / 倒计时 = 还能不能撤），并把右键那一行补成
      `cancel the pending action (stop) / give up on a threat`。
      面板从 12% 下移到 6% 并加 `max_height` + `overflow: clip_y`（多加 6 行后
      720p 上会顶到屏幕底边）。
      **验收**：`the_help_teaches_the_core_loop`（逐项断言那五个判据 + 右键要写
      `give up`）；删掉那几行会红。

- [x] **#55 技能槽是假按钮**（已修）
      拍板走「甲：点击 = 只选中」，实现放 **`input` 域**（`input ──▶ presentation`
      这条依赖本来就有；`interaction` 的域描述里不含 `presentation`）。
      `src/input/pointer.rs::skill_slot_click_input_system`：`Changed<Interaction>` →
      `Pressed` → `SelectSkill(index)`，**不释放技能**。
      **验收**：`clicking_a_skill_slot_selects_it_without_casting`——选中第 3 格、
      **没有** `UseSelectedSkill` / `PlayerTakeover`，且悬停不算点击。
      ⚠️ **实机待补**：点第 3 格 → 金色描边跳过去、玩家原地不动。

- [x] **#61 多敌人面板的溢出计数 + 布局重叠**（已修）
      ① `UnitPanels` 加 `dropped_enemies` + `overflow_line()`（`还有 N 个`，没有溢出时空串）；
      ② 敌人列顶上加一行 `PanelText::EnemyOverflow`（池化节点，没溢出时 `Display::None`）；
      ③ 敌人列改用 `Column` + `justify_content: End`（**不再用 `ColumnReverse`**：它把主轴
      起点也翻过来，生成顺序与视觉顺序相反），并给列与日志都加显式高度上限；
      ④ **修掉实测到的重叠**：`CombatLog` 的 `bottom` 以前写死 126px，而敌人列加了
      Focus 行 / 溢出行之后会长高——现在由 `LOG_BOTTOM_CLEARANCE`
      （`14 + ENEMY_COLUMN_MAX_HEIGHT + 8`）算出来，两块几何上不可能重叠。
      **验收**：`enemies_beyond_the_row_pool_report_how_many_were_dropped`、
      `the_enemy_column_can_hold_a_full_pool`、`the_combat_log_clears_the_enemy_column`、
      `a_row_is_tall_enough_for_its_content`。
      ⚠️ **实机待补**：要 4 个敌人才看得到「还有 1 个」（现在开局只出 2 个）。

- [x] **#50 手动暂停时时间轴仍显示 `RUNNING`**（已修）
      **根因**：`timeline/system.rs` 只读 `reasons.is_frozen()`，而 `PauseReasons` 里
      **永远没有 `manual`**（`Toggle` 不带原因，设计如此）。
      **改法**：① `clock` 加只用于显示的判断（读 `ManualPause`，**不进集合**——
      `is_frozen()` 保持不动，时钟判据本来是对的）；② HUD 拼状态行时把 `manual`
      补进原因列表；③ `PauseReasons` **不派生 `Reflect`**（内含 `HashSet`），
      改为新增只读镜像 `PauseLabels` 供 BRP 查（见 [`clock.md`](clock.md) 的 #62）。
      **验收**：`the_state_line_reads_the_freeze_reasons`（含 `manual + threat` 组合）
      ——注意旧版那条用例**直接喂 labels**，验不到这个 bug；新用例从资源走了一遍。
      **实机**：BRP 读到 `ManualPause = false` / `PauseLabels = ["awaiting"]`，
      状态行 `TIMELINE · FROZEN · awaiting`。

## 还没做（本文件里剩下的）

- [x] **[P2][bug] 敌人面板的 `act:` 行是「按阵营」而不是「按行」**（2026-09-27 已修）
      **现象**：三个敌人行（`MAX_ENEMY_ROWS = 3`）显示**同一句话**——都是"第一个敌人"
      的动作。实测证据：场上只有 **2** 个敌人时，第三个（空）行的 `ActionLabel`
      也在显示 `act: fireball (windup 0.3s)`；四个 `ActionLabel` 实体
      （1 玩家 + 3 敌人）里三个敌人标签文本**逐字相同**。
      **根因**：`panels/scene.rs` 给每行挂的是 `ActionLabel { faction }`——**只带阵营、
      不带行号**；`actions.rs::update_action_labels_system` 又按 `slot(label.faction)`
      写快照（一个阵营一格），于是同一阵营的所有行拿到同一句话。
      而 HP / EN / Focus / Insight 那些读数**都是按 `PanelSlot` 分行的**，
      所以这是面板内部唯一一处"行列不对应"。
      **为什么现在才暴露**：两个敌人常常在做同样的事（同一套 AI、同样的距离），
      那时这个 bug 看不出来；要它们动作不同才显形。
      **改法（按计划执行）**：把动作文案**并进面板模型**，与其它读数同源——
      ① `UnitRow` 加 `action: Option<ActionReadout>`（动作名 + 前摇剩余），
      `PanelText` 加 `Action(PanelSlot)`，格式化落在
      `UnitPanels::action_text(slot)`；② 面板系统在造 `UnitRow` 时按**实体**算出它
      （它本来就有"行 → 实体"的映射）；③ `ActionLabel` 组件、
      `ActionLabelCache`、`update_action_labels_system` **整体删除**；
      `payload_name` / `PayloadQueries` 留下不动（它仍是"面板与时间轴共用一套命名"
      的那处真相源）。
      **副作用（好事）**：`update_unit_panels_system` 的参数因此到 17 个、
      超过 Bevy 的 16 上限，于是把 `dodging` / `parrying` / `airborne` 三个只读标记
      查询收成 `StateMarkers`（`SystemParam` 派生）——顺带让签名短了两行。
      **验收**：`each_row_shows_its_own_action`（两个敌人不同动作 → 两行不同；
      空行 `act: -`）+ 实机确认：空行从"显示别人的火球"变成 `act: -`，
      两行各自读自己那行，布局不变。

- [ ] **[P2][ux] 敌人面板可展开详情**（原 issue #59，`docs/insight.md` 第四节「乙」）
      **已做**：每行已有 `range 1 · break 1 · approach`。
      **未做（本条）**：可展开 / 收起。**这是主动放弃的那一半**——实现时判断
      "直接列出来更简单也更有用"。
      **触发条件**：读数多到一行放不下（比如加入"会什么技能"整列）时再做；
      届时的改法是照抄 `log_panel` 的折叠机制（`Button` + `Collapsed(bool)` +
      `Changed<Interaction>`），**不要新造机制**。

- [ ] **`HudCache` 缺 `Reflect`，HUD 快照无法远程诊断**（2026-09-27 记，P3）
      `HudCache` 装着 `TimelineCache` / `UnitPanelCache` / `SkillBarCache` / `LogCache`，
      是"这一帧 HUD 认为世界是什么样"的**唯一快照**，但它没派生 `Reflect`，
      所以 BRP 读不到——想数值核对时间轴的 `lanes` / `ready` 只能退回读 `Node`（啰嗦）
      或截图（不精确）。**代价**：一串子结构都得跟着派生（`TimelineSlot` / `ActionRow` / …），
      还要加进 `the_diagnostic_anchors_are_reflected`。**收益**：HUD 从"只能看"
      变成"可查询"，这一类"屏幕在说谎"的 bug 会好查很多。
      **先不急**：本轮用"截图 × 决策槽交叉核对"已经够用，等下次再被卡住时再补。
