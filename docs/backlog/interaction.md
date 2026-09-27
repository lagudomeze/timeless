# backlog · interaction（拾取 / 点击 / 悬停 / 预演 / 世界空间指示）

> 活的总览：[`../../TODO.md`](../../TODO.md) · 历史证据：[`../../CHANGELOG.md`](../../CHANGELOG.md)

- [ ] **[P1][feat] 洞察力面板：敌人面板可展开详情**（原 issue #59，`docs/insight.md` 第四节「乙」）
      **已做**：敌人面板每行已有洞察力读数 `range 1 · break 1 · approach`
      （射程 / 打断抗性 / 战术），`break` 只在敌人真有前摇中的那一手时出现。
      **未做（本条）**：可**展开/收起**的详情区——策划稿把它当作「乙」的代价，
      实现时判断"直接列出来更简单也更有用"，于是**主动放弃**了展开这一半。
      **改法（若要做）**：照抄 `log_panel` 已经跑通的那套（`Button` + `Collapsed(bool)` +
      `Changed<Interaction>` 切换系统），**不要新造机制**；详情区只在展开时写（沿用快照比对）；
      `layout.rs` 的具名节点表补上详情区节点（它同时是 BRP 的锚点表）。
      **验收**：模型层每个读数的文案用例（含"没有该组件的单位"的退化）；
      交互用例照 `clicking_the_header_collapses_the_log` 的写法：点标题展开 → 读数出现 → 再点收起。
      **触发条件**：当读数多到一行放不下（比如加入"会什么技能"整列）时再做；
      在此之前「直接列出来」是对的，别为了对齐策划稿而重新加一层交互。

- [ ] **[P2][ux] 技能槽点击的域归属收口**（原 issue #55 的可选清理，**本次不做**）
      条目本体（点击 = 选中）在 [`hud.md`](hud.md) 里。这里只记那条可选的搬家：
      `input → presentation::hud::skills::SkillSlot` 这条依赖看着脏，
      可以把 `SkillSlot { index }` 这个标记类型挪到 `combat::attack`（技能目录的领域）
      当"槽位身份"，HUD 只负责画。**纯搬家、不改行为**。
      **触发条件**：出现第二个"要按槽位号做事的域"时再做（现在只有输入域需要它）。

## 已完成的条目（证据见 CHANGELOG）

- [x] **#46 UI 门控**：`hover_cell_system` 在 `PointerOverUi` 为真时把 `HoveredCell`
      写成 `None`（**必须清空**，否则悬停高亮与 AOE 预演会留在世界里）；
      `pointer_command_system` 丢弃落在 UI 上的左/右击。
      ⚠️ **这条没法用 BRP 验**（2026-09-27 实测）：BRP 的合成光标驱动不了 Bevy 的
      UI 焦点——7 个区域根节点的 `RelativeCursorPosition.cursor_over` **全为假**，
      于是 `PointerOverUi` 恒为假、"点面板世界不动"验不出来；**只能真人用真鼠标**。
      详见 [`../playtest-checklist.md`](../playtest-checklist.md) 第 3 节与「已知坑 #2」。
      **未做（可选）**：中键拖拽 / 滚轮也按同一判据门控——现状是拖着面板也能平移镜头，
      无害但不一致。想统一时把 `PointerOverUi` 读进相机系统即可。
- [x] **#57 威胁格可视化**（主体）→ `presentation/threat_grid.rs`，池化 8 格，
      `Threatens.cells` + 飞行中投射物的 `TargetCell` 画在地面上；
      **只在反应窗口开着时画**（关窗即隐藏，表现层只读 `ReactionSlot`，不自己判断）。
      **未做**：威胁**来源圈**（敌我脚下一圈威胁色）——它与阵营环共用零件，
      见 [`presentation.md`](presentation.md) 的 #56。
