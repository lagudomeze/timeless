# Project Timeless — 变更日志

> **本文件是已完成工作的证据档案，写完即冻结。**
> 活的待办见 [`TODO.md`](TODO.md)，按领域分的条目见 [`docs/backlog/`](docs/backlog/)。
> 勾选规则：必须有验收证据（命令输出 / 测试名 / 控制台片段）才允许 `[x]`——
> 下面每一条都是按这个规则写的，因此**不要为了"整理"而删掉证据句**
> （测试名、实机读数、踩过的坑才是这里的价值）。

---

## 已完成

### 无回合重构 + 能力迁移（M1–M15）

- [x] **M1 时间线地基**：`Ready` / `BusyRecovery` / `ActionTiming`；
      删除 `Phase` / 轮次 / 1s 窗口 / `RoundEnded`；系统链 = 门控 → 提交桥 →
      调度 → 后摇恢复。
- [x] **M2 格子移动**：`Cell` / `MoveGoal` / `step_from_axis`；按一次走一格、
      到格中心吸附停下。
- [x] **M3 资源与防御**：`Stamina`（恢复 `Ready` 时 +1）、翻滚
      （1 精力 / 退一格 / 0.5s 无敌帧）、招架（1 精力 / 免伤 + 一半反制）、
      防御判定插在伤害之前、防御标记过期清理。
- [x] **M4 火球**：锁目标格、自由飞行、到达后按真实距离结算 12 点 AoE
      （半径 1.5 格）；空地爆炸完全落空。
- [x] **M5 两阶段结算**：领域层纯逻辑（三层裁决帧 → 真实距离 → 破势、
      防御判定、反制伤害）+ `phase1_arbitrate`（只读）→ `phase2_apply`（统一落地）。
- [x] **M6 AI 意图循环**：选意图与声明行动拆成两个系统；六种意图（含 `Dodge`）；
      威胁用 `CollisionTarget` 判定。
- [x] **M7 技能菜单与 HUD**：`SKILLS` 注册表（单一来源 + 精力可用性过滤）、
      `MenuSelection` + 选择 / 循环 / 派发（`Attack` 按真实距离派发近战或火球）。
- [x] **M8 单位 2D 纸片 + 贴地阴影**：billboard 绕 Y 轴对准相机，
      高度用正下方地表上的黑色阴影表示（Kenney Tiny Dungeon，CC0）。
- [x] **M9 HUD 重构**：五块布局（时间轴 / 双方面板 / 技能栏 / 可折叠日志 / 帮助）；
      `UiScale` 按窗口高度适配；`HudCache` 快照比对（内容没变就整帧不碰 UI）。
- [x] **M9.2 移动手感**：新增 `follow_terrain_system`（贴地）与
      `end_action_until`（忙到真正到达目标格）；时间轴色块从 `declared_at` 长出去。
- [x] **M9.3 以 PC 为中心**：出生点用格中心（`spawn::cell_ground`）、
      相机跟随 PC（中键拖拽 = 观察偏移）、`ActionBlocked` 提示条。
- [x] **M9.4 地形量化到决策格**：`TERRAIN_CELL = 2`，整格 2×2 体素共享一个高度，
      台阶只出现在格边界。
- [x] **M9.5 时间轴分道 + 候场区**：每个单位一行、右侧显示「已就绪还没声明」的人。
- [x] **M9.6 AI 复活 + 行动归属**：`EnemyBrain` 用 `#[require(Intent)]`；
      AI 的 `Dodge` 直接生成自己的 roll 行动，不借玩家的 `RollCommand`。
- [x] **M10 鼠标 Raycast + 悬停高亮**：`interaction` 新域，沿地形高度场步进的纯函数拾取。
- [x] **M11 点击交互**：左键点地板走 / 点单位用技能、右键撤销；
      火球改成执行时才发射（撤销不留半空火球）。
- [x] **M12 预演指示器**：火球 AOE 圆盘 + 近战扇形，判据与派发系统一致。
- [x] **M13 预演读数 + 滚轮缩放**：提示条显示「技能 · 格 · 距离 · 预计伤害」；
      滚轮改 `CameraRig.zoom`。
- [x] **M14 打断 / 撤销 / 取消代价**：`WASD` 让给技能热键、数字键改成直接放技能、
      `F2` 循环反应窗口（`Loose` / `Strict` / `Off`）、`interrupt_system`、
      `ActionCost` / `CancelCost` / `Uncancellable`。
- [x] **M15 同帧阵亡崩溃 + 远射被冻在半路**：收尾全程走 `Commands::get_entity` 守卫；
      火球用 `end_action_until(.., flight_time)` 忙到落地。
- [x] **M16 时间戳 + 事件模型**：删掉 `Ready` / `BusyRecovery` /
      `Declared` / `Pending` / `Committed` / `ActionCost` / `CancelCost` /
      `Uncancellable` / `Impact` 与 `Arbitration` 两阶段结算，改为
      「决策槽 + `ScheduledAction.execute_at` + 后摇」：
      执行器自己判 `due()`、自己销毁行动实体、自己写行动者的后摇（无 `scheduler_system`、
      无 `begin_action` / `end_action`）。暂停改成**原因集合**（`"manual"` /
      `"awaiting"` / `"threat"`，唯一的时钟写入点是 `apply_clock`，空格不再只前进一帧）；
      新增**反应系统**（`Threatens` / `TargetCell` → `detect_threat_system` → 冻结等玩家表态）
      与 **Focus**（Shift + 决策键：扣 1 点把前摇归零）；打断改成
      `InterruptEvent`（EntityEvent + Observer，掷骰对抗打掉**还没到点**的行动）；
      伤害压成一条链（`DamageEvent` → `apply_damage_system` → `DeathEvent` →
      `despawn_dead_system`，血量改整数、扣到负数继续扣、死亡只报一次）。
      验收：169 测试全绿 / clippy 零警告 / `rg "ResMut<Time<Virtual>>" src` 只命中 `apply_clock`。
- [x] **M17 阶段收口 + 交互边界 + 归属交给关系**（四次提交 d94bc0b / 7e47a05 /
      faa3f5a / 970313a）：① `DecisionSlot` 变成**三态**
      （`Empty` / `Windup` / `Recovery { until }`，住在新的 `decision.rs`），
      `Busy` 组件与 `DecisionSlot::Filled` 删除；调度数据搬进新的 `schedule.rs`。
      ② 交互边界：`F5` 的读取从 `spawn/restart.rs` 搬进
      `input::keyboard::restart_input_system`；时间线不再读各领域的命令，
      改认输入层唯一的一条 `PlayerTakeover`（写：键盘 / 左键，消费：`interrupt_system`）；
      `recovery_system` 不再直接改 `Stamina`，改为 trigger `DecisionReady`
      （`combat::defense::recover_stamina_observer` 订阅回 1 点）；暂停改成**每帧断言**
      （`PauseRequest::{Pause, Toggle}`：`Pause` 每帧断言、`Toggle` 清空/停住，`process_pause_requests`
      每帧先 `clear()`，删掉 `TogglePause` / `ManualPause` / `compute_manual_pause` /
      `request_on_edge`），手动暂停的闩搬进 `input::keyboard::pause_input_system`。
      ③ 撤销退款：`Cancellable` 枚举删除 → `timeline::Uncancellable` 标记组件
      （`undo_system` 用 `Without<Uncancellable>` 过滤）；`ActionCancelled` 从 Message
      改成 **EntityEvent**（`{ entity, actor }`，在 `despawn` **之前** trigger）；
      退款搬到花钱的领域（`refund_fireball_observer` 退 2 收 2、
      `refund_melee_observer` 收 `MELEE_CANCEL_PENALTY`，删掉
      `refund_cancelled_actions_system`）；翻滚 / 招架执行时才扣精力，撤销不退款。
      ④ 行动实体成为行动者的**子实体**（`ChildOf`，`ScheduledAction.actor` 字段删除）：
      声明侧统一 `spawn_scene().id()` + `add_child(action)` + `DecisionSlot::Windup`，
      读取侧（七个执行器、`undo_system`、`interrupt_observer`、`detect_threat_system`、
      HUD 时间轴与行动行）改带 `&ChildOf` 取 `parent()`；父节点销毁时行动跟着销毁，
      因此 `reset_battle_system` 的清场查询删掉了 `With<ScheduledAction>`。
      验收：178 测试全绿（176 单元 + 2 资产验收）/ clippy 零警告 / `cargo fmt --check` 通过。
- [x] **M18 节奏与格尺度归位 + 调度数据瘦身**（两次提交 3c043c7 / 本次）：
      ① `timeline/timing.rs` 只留 `ActionTiming` 这个**形状**，六个具体数值搬到载荷旁边
      （`MOVE_TIMING` / `JUMP_TIMING` / `ROLL_TIMING` → `movement/actions.rs`、
      `MELEE_TIMING` / `ARROW_TIMING` → `combat/attack/actions.rs`、
      `FIREBALL_TIMING` → `combat/attack/fireball.rs`、`PARRY_TIMING` →
      `combat/defense/actions.rs`）。动机是时间线自己的承诺「新增动作时调度器一行不改」
      在此之前**是假的**——加一个动作必须回头改 `timeline/timing.rs`；旁证是
      `SHOOT` 同时服务两个载荷、`ROLL` 被两个领域引用、`timeline/schedule.rs` 的单测
      被钉死在具体载荷上。
      ② `CELL_SIZE` 从 `timeline/timing.rs` 搬到 `movement/cell.rs`：`timeline/` 里
      本来没有任何代码用它，真正的定义方是 `Cell` 的格 ↔ 世界换算，消费方是战斗射程、
      地形量化（`world::TERRAIN_CELL` 的注释原本写着"必须等于 `timeline::CELL_SIZE`"）与鼠标拾取。
      ③ `ActionTiming` 变成**行动实体上的组件**（场景工厂跟着载荷一起挂），
      `ScheduledAction` 因此瘦成 `{ execute_at, interrupt_resist }`；
      HUD 时间轴色块改成从 `execute_at − timing.windup` 起算、宽度 `timing.total()`。
      ④ 调度器与反应系统的单测改用自造的 `TEST_TIMING`，不再被具体载荷钉住。
      验收：178 测试全绿 / clippy 零警告 / `cargo fmt --check` 通过 / `cargo run` 无 panic。
- [x] **M19 调度域归位：打断判定搬去战斗域**（提交 9a282a6 / 本次）：
      审计 `timeline/` 时发现「命中能不能打掉那一手」这套**战斗裁决**（`power + 3 + 3d5`
      对 `interrupt_resist + 3 + 3d5`）内联在时间线的 Bevy Observer 里，直接违反
      AGENTS.md 的「领域层 `combat/formula/domain.rs` 绝不引入 Bevy；应用层不包含伤害公式」
      ——同族的 `resolve_defense` / `counter_damage` 都是那里的纯函数，只有它漏在缝里。
      ① `interrupt_observer` + 骰子 `roll_3d5` 整体搬到 `combat::formula`，
      `InterruptEvent` 跟着走（生产者与消费者现在同属战斗域）；时间线不再注册它。
      ② 算式提成纯函数 `combat::formula::domain::interrupt_lands(power, resist, 骰, 骰)`
      并补单测——顺便测出那 3 点 `INTERRUPT_BASE` 在不等式两边同时出现、**对结果没有影响**。
      ③ `ScheduledAction` 因此只剩 `{ execute_at }`：打断抗性本来就是 `ActionTiming`
      （载荷节奏）的一部分，而两者挂在**同一个行动实体**上，不必再抄一份快照。
      ④ 删掉死方法 `PauseReasons::remove`（暂停原因改「每帧重建」后只剩测试在用它）。
      ⑤ `timeline::FirstReady::first_ready` 成为**声明的唯一入口**（迭代器上的方法，
      槽的位置由 `HasDecisionSlot` 回答——约定放在查询元组末位，实测零 GAT/推断摩擦）：
      「槽必须是 `Empty`」这条判据与「被拒时报 `ActionBlocked::BUSY`」原来在 9 个声明系统里
      各写一遍（其中 8 个是活路径、1 个是当时未注册的 `declare_skill_system`；
      后者现已由 `declare_shoot_system` 取代并删除），现在收成一处。
      验收：181 测试全绿（179 单元 + 2 资产）/ clippy 零警告 / `cargo fmt --check` 通过 /
      `cargo run` 无 panic。
- [x] **M20 时间线整理：声明的另一半 + 按概念分文件**（提交 57df65e / 本次）：
      ① `timeline::attach_action(commands, actor, action)` 补上"声明"的另一半——
      `first_ready` 管「谁」，它管「账怎么记」（挂成行动者的子实体 + 决策槽推进 `Windup`）。
      这两句原来在 **8 处**一字不差，且横跨玩家路径与 AI 路径，其中三个是两边共用的工厂。
      **（这一条随后被 M21 撤掉了：`attach_action` 把"占槽"这次状态转换藏了起来。）**
      ② `timeline/` 从 **9 个文件收到 7 个**，每个文件回答一个问题：
      `decision.rs`（谁能决策 + 决策的一生：`undo_system` / `recovery_system`）、
      `schedule.rs`（这一手何时落地：`ScheduledAction` / `ActionTiming` / `Uncancellable`）、
      `clock.rs`（世界何时冻结：`PauseRequest` / `PauseReasons` / 三个原因常量 + 三个系统）、
      `focus.rs`（⚠️ Focus 一族，**待搬去 combat**）、`events.rs`（其余跨领域契约）。
      删掉 `components.rs`（23 行碎片）/ `timing.rs`（49 行碎片）/ `systems.rs`（8 个不相干的
      系统堆在一起）；**系统跟着它操作的数据走**，AGENTS.md 的文件约定同步改成
      「按概念分文件，系统跟着数据走」。
      ③ `interrupt_system` 并进 `undo_system`：它的名字骗人（真正的打断 `InterruptEvent`
      已搬去 `combat`），实际做的是"玩家表达了新意图 → 撤掉旧那一手"。现在 `undo_system`
      直接收两个来源（`UndoCommand` 右键 / `PlayerTakeover` 换手），少一条消息转发。
      验收：181 测试全绿 / clippy 零警告 / `cargo fmt --check` 通过 / `cargo run` 无 panic。
- [x] **M21 撤掉 `attach_action`：让场景工厂自带 `ChildOf`**（本次）：
      M20 ① 把「挂成子实体」与「决策槽推进 `Windup`」打包成一个 `attach_action`，
      但后者是**整个模型最关键的一次状态转换**（`decision.rs` 自己写着"转换只有五条路，
      每条都只有一个作者"），打包之后读声明系统的人看不出"这里把玩家的决策槽占了"
      ——为省 8 处各 1 行牺牲了可审计性，不划算。改成让**实体出生时**就成立：
      7 个行动场景工厂各多收一个 `actor: Entity`，把 `ChildOf({actor})` 直接写进 `bsn!` 模板，
      声明点于是不再需要 `add_child`，而 `insert(DecisionSlot::Windup)` 恢复在每个声明点明写。
      **顺带纠正一个我的错误推断**：我以为 BSN 写不了 `ChildOf`（`Template` 只有
      `impl<T: Clone + Default + Unpin>`，而 `ChildOf` 没有 `Default`），实测
      `bsn! { ChildOf({actor}) ... }` **可以编译且语义正确**（`the_world_keeps_making_progress`
      证明 AI 的行动确实被链到行动者）。
      验收：181 测试全绿 / clippy 零警告 / `cargo fmt --check` 通过 / `cargo run` 无 panic。
- [x] **M22 关系模型：行动归属从 `ChildOf` 换成自定义关系 `ActionOf` / `Actions`**（本次）：
      `ChildOf` 是**空间层级**（父 `Transform` 传给子级），而行动实体连 `Transform` 都没有；
      拿它表达归属会让纸片、阴影、行动三种完全不同的东西挤进同一个 `Children`
      ——「谁是谁的孩子」这句话直接不可读。新增 `src/timeline/ownership.rs`
      （一个文件回答一个问题：这一手是谁的），`Actions` 标 `linked_spawn`，
      所以「行动者阵亡 / 重置 → 名下行动跟着销毁」这条结构性保证一分不少。
      **先实测后动手**（M21 的教训）：`bsn!` 认的是「组件有没有派生 `FromTemplate` +
      字段有没有 `#[entities]`」，和"是不是内置的 `ChildOf`"无关——`bsn! { ActionOf({actor}) }`
      一次编译通过，`Actions` 的 hook 维护与级联销毁都有用例守着（3 条新用例）。
      改动面：7 个场景工厂 + 10 个读取点（执行器 / 撤销 / 打断 / 威胁 / HUD）
      （`child_of.parent()` → `action_of.actor()`）+ 20 处测试夹具；**行为不变**。
      验收：182 测试全绿（179 + 3 新）/ clippy 零警告 / `cargo fmt --check` 通过 /
      `cargo run` 无 panic。
- [x] **CJK 字体**：`assets/fonts/NotoSansSC-Regular.otf`（OFL-1.1），
      HUD 显式指定，战斗日志中文不再显示成豆腐块。
- [x] **文档整合**：文档收敛为「入口 + 架构 + 时间线 + 组件对照 + 设计 + 素材 +
      Bevy 速查」七篇，进度合并进本文件；移除已冻结的 `timeless/` workspace。

## 设计落地路线（M22+，本轮定稿）

> 一次设计评审的结论。**文档已按目标态重建**（`docs/index.md` 是新入口），
> 代码正按下面的顺序往上落（M22 已完成）——所以每一条都是"文档已写、代码待落"，
> 目标态与现状的差异在每篇文档顶部都标了 ⚠️ / 🚧。

**逐条定夺的结果**（8 条冲突 + 3 条结构性决定）：

| # | 主题 | 决议 |
| :--- | :--- | :--- |
| C1 | 行动的归属 | **换成自定义关系** `ActionOf(actor)` / `Actions`（`linked_spawn`），不再用 `ChildOf`——行动没有 `Transform`，"物理附着"这个词不该被挪用 |
| C2 | 决策槽 | **换成意图式** `Idle { intent } \| Executing { until }`：槽回答"决定了没有"，阶段由行动实体回答 |
| C3 | 打断 | **标签做闸门 + 掷骰做对抗**：`interruptible` / `super_armor` 决定能不能断，`interrupt_lands` 决定这一次成不成 |
| C4 | 格挡 | 采纳（防御链补一格：闪避 → 招架 → 格挡 → 抗性 → 扣血 → 打断） |
| C5 | 威胁 / 反制 | 采纳 `ReactionSlot` + `CounterSuggestion` + `CounterCost`，**取代 `ThreatWindow`**（顺带修掉那个死锁） |
| C6 | 释放条件 | 采纳 `AbilityDef.requirements` + 集中 `can_cast()`；`phases` / `effects` 后置 |
| C7 | 装备 | 采纳（槽位 `ChildOf` PC + 物品 `EquippedTo` 槽位），纯新增 |
| C8 | 关系模型总纲 | **物理附着用 `ChildOf`，逻辑关系用自定义关系**（新铁律） |
| — | `ActionQueue` | **删除**：决策槽 + 时间轴已覆盖"收集决策、排序执行" |
| — | 目录粒度 | **每个 mod 出自己的 `plugin.rs`**，父域只编排顺序 |
| — | 文档 | **重建**（不增补）：新增 domain / relations / combat / skills / equipment；`architecture.md` / `components.md` / `NEW_DESGIN.md` **已替代完成并删除**（M29） |

**C2 的两个技术发现**（写进 `docs/timeline.md` 第三节）：

1. **"同时提交"是免费的**：冻结时 `Time<Virtual>` 不走，① 和 ③ 都以同一个
   冻结时刻为基准——思考 5 秒和 0.5 秒起跑线相同。不需要额外的提交动作。
2. **`{ intent, executing }` 装不下后摇截止**：行动实体执行时就销毁了，
   所以槽需要 `Executing { until }` 带上"忙到什么时候"。

### 追加决议（评审第 2 轮，2026-09）

评审对上面的方案提了 6 条修正 + 一轮之内到底发生什么。**已全部写进文档**：

| # | 主题 | 决议 |
| :--- | :--- | :--- |
| D1 | 命名 | `ai::Intent` → **`ai::Tactic`**（战术），`ai::Decision` → **`ai::Situation`**（战况），`timeline::PlayerIntent` → **`PlayerTakeover`**（玩家动手了），`FocusIntent` → **`PendingFocus`**；`Intent` 这个词**只留给"动作决策"** |
| D2 | 技能 | **`skills` 抽成顶层域**（不再挂在 `combat` 下）；**移动 / 跳跃 / 翻滚也是技能**，不做特殊处理；`AbilityDef` 是**静态、可序列化**的那一半（不许出现 `Entity` / 闭包），各机制域通过 `RegisterAbility` 把自己的定义交上来；`combat/attack` 改名 **`combat/attack`** |
| D3 | 范围 | `Shape` 只是几何：住 **`utils`**，不是领域、没有 Plugin；伤害 = `Point`、回血 = 正方形；具体实现**直接引用或包一层**，不抽象成机制 |
| D4 | 反制 | `CounterSuggestion` = **所有能当反制的技能 + 它要付的反制资源**，也就是 `ReactionSlot` 的 UI；HUD 高亮"付得起"的那些 |
| D5 | 属性叠加 | 「基础值 + 加成」的结构**排在技能静态定义之后**定（`can_cast` 与命中公式都要读属性） |
| D6 | 本轮范围 | **先把战斗做完**；roguelike 那一半（供应链 / 信息 / 掉落）只有 `game-design.md` 的总纲 |

**一轮之内发生什么**（取代我原来写的"闸门 / 开闸"两个概念）：

```text
① 非 PC 的决策   所有空决策槽 → can_cast → 填 Intent → 立刻物化
② 威胁扫描       前摇中、玩家还没表态的行动 → 与 PC 所在格相交 → 记入 ReactionSlot
③ PC 的判定      槽空 → Pause("awaiting")；有威胁且没表态 → Pause("threat")
                 + 高亮有反制资源的技能
④ 结算           到点 → 效果判定（命中 / 防御链 / 打断 / 扣血）
```

**没有"闸门"这个机制**：能不能决策由决策槽回答（③），效果能不能落地归结算期的
效果判定（④，见 `docs/combat.md` 第一节）。一帧的顺序（`InputSet → AiSet →
执行器 / 威胁扫描 → 帧末 ClockSet`）**是语义的一部分**，不是性能选择。

### 待办（按顺序）

- [x] **M22 关系模型**：行动归属从 `ChildOf` 换成 `ActionOf` / `Actions`（`linked_spawn`）。
      **先实测 `bsn!` 能不能写自定义关系组件**——M21 那次"按 `Template` 约束推断不行、
      实测却可以"的教训要记住。改动面：7 个场景工厂 + 所有 `child_of.parent()` 读取点
      （执行器 / 撤销 / 打断 / 威胁 / HUD）+ 两个测试。行为不变（级联销毁保留）。
      **已落地**：新增 `timeline/ownership.rs`；实测 `bsn!` 可以，机制是 `FromTemplate`
      + `#[entities]`（见 `docs/relations.md` 第五节）。
- [x] **M23 前置：命名归位（D1）**：`Intent` 这个词原本被三个东西占着，填意图之前先分清——
      AI 的 `Intent` 是**战术**、`PlayerIntent` 是**输入层事实**、`FocusIntent` 是**本帧请求**，
      三者都不是"决策"。改名：`ai::Tactic` / `ai::Situation` / `PlayerTakeover` /
      `PendingFocus`（+ `track_pending_focus_system`、`tactic_label`），顺带修掉
      `src/` 里 5 处多了一层 `..` 的文档链接。**行为不变**（纯改名，编译器全程把关）。
      验收：182 测试全绿 / clippy 零警告 / `cargo fmt --check` 通过。
> **顺序调整**：`Intent { ability: AbilityId, target: Target }` 里的 `AbilityId` 恰好是
> M24 的产物，所以**目录先立、槽再引用它**（D2 的答案已经蕴含这一点）。
> M24 拆成两块：**M24a 静态目录**（已落地，纯增量）与 **M24b `can_cast` + 菜单迁移**。

- [x] **M24a `skills` 顶层域 + 静态目录（D2）**（本次）：`src/skills/` 立起来——
      `AbilityId` / `AbilityCategory` / `AbilityDef` / `CombatTags` / `TargetSelector`、
      `SkillRegistry` + `RegisterAbility`（各域在 `Startup` 交定义，目录每帧并进；
      重复注册按 id 覆盖且**不动菜单顺序**）。`movement` 交 `Move`/`Jump`/`Roll`、
      `combat` 交 `Melee`/`Shoot`/`Fireball`/`Parry`——**数值仍归各域**，目录只聚合。
      两条纪律写进了代码：`AbilityDef` 里不许有 `Entity`/闭包（否则 `.ron` 这条路断掉）、
      **没有全局派发器**（谁声明谁物化）。
      踩到一个接线坑，值得记：只装 `MovementPlugin` 或 `CombatPlugin` 的单测会因为没有
      `Messages<RegisterAbility>` 直接 panic；解法是让**写方插件自己补上 `SkillPlugin`**
      （它确实依赖目录），而不是让读方到处 `Option<MessageWriter>` 兜底。
      **行为不变**（旧的 `SKILLS` 菜单暂时留着，M24b 再迁）。
      验收：193 测试全绿（191 单元 + 2 资产）/ clippy 零警告 / `cargo fmt --check` 通过 /
      `cargo run` 无 panic。
- [x] **M24b `can_cast`**（本次）：`Requirement`（只列真的会检查的：`EnoughEnergy`）+
      `can_cast(def, stamina)` 收成唯一的条件校验点——菜单过滤 / `fireball` / `roll` /
      `parry` / AI 的"闪不闪得动"全走它，失败复用 `timeline::BlockReason`。
      **菜单与目录的对账**由 `menu_matches_the_catalogue` 钉住（花费 / 节奏 / 威力分叉即红）。
      两处偏离原设计并写进了 `docs/skills.md`：`Requirement` 不预先堆十条、
      `can_cast` 收事实而不是 `&World`。
      验收：221 测试全绿（219 单元 + 2 资产）/ clippy 零警告 / `cargo fmt --check` 通过。
- [x] **执行器按"看到的那一帧"算后摇 → 忙碌窗口多 0~1 帧**（已修）：`recovering` 现在收
      `&ScheduledAction`，以 `execute_at` 为基准（不再用"发现到点的那一帧的 `now`"）。
      8 个执行器全部改过。验收：1s 的等待实测 `until = 1.0`（之前 1.1），
      第 11 帧（0.1s×11）回到 `Idle`；
      `the_recovery_window_does_not_depend_on_when_the_executor_notices` 钉住基准。
- [x] **等待时长要可配**（本次）：`WAIT_SECONDS` / `WAIT_TIMING` / `WAIT_ABILITY`
      三个常量收成一个 **`WaitConfig` 资源**（默认 1.0s）——声明时现算节奏、
      交上去的技能定义从它派生、帮助面板不再写死秒数。
      **只此一处真相**：三个消费者（声明 / 目录 / HUD）都从配置读，改一处全都跟着变。
      验收：`the_wait_duration_comes_from_the_config`（把配置调成 3s，声明出来的
      行动就忙 3s）与 `the_registered_definition_tracks_the_config`；
      前者**验过不是空跑**（改回硬编码 1.0 后转红）。
      等动作数值整体外置（`.ron`）时，这个资源就是要序列化的对象。
- [x] **暂停：情形 A 已由「等待」动作解决**（`feat/wait-action`，已合并）：玩家空闲时按空格
      生成一条占槽 1s 的等待行动 → `awaiting` 消失 → 世界继续跑。
      **`ManualPause` 因此只服务"忙碌 + 运行"那一种情形**（他没有槽可占）。
      `ManualPause` 的引入**已被证明是必要的**：`clock` 的测试
      `the_manual_pause_survives_a_domain_reason_disappearing` 就是钉这条语义的。
      验收：`cargo test` 的 `space_asks_for_a_wait_not_a_pause` / `p_toggles_the_manual_pause`。
- [x] **威胁：按 action 上报一次**（M26 落地的形态与这里设想的不同）：原计划
      "威胁是 action 的属性"，实际做成了 **`ReactionSlot` 挂在被威胁的玩家身上**
      （`threat` + `suggestions` + `resolved`），表态走显式的
      `ReactionAnswer::{Counter, Abandon}`——比"改属性"更直接地解决了同一个问题：
      同一个 `threat` 实体只开一次窗，表过态就 `resolved`，**不再靠全局记账**。
      验收：`combat::reaction` 的用例（表态后再冻结必然是**新威胁**）。
- [x] **M24c 菜单改读目录 —— 审计结论：不做整体迁移，只清掉一处真重复**（本次）：
      先说**为什么不做**：`SKILLS` 是"菜单栏"（4 格、有顺序、数字键绑定、HUD 按
      **定长数组**存快照 `[bool; SKILLS.len()]`、`[CounterHint; …]`），目录是"技能本体"
      （7 条）。按 `AbilityId` 重建菜单会涉及 **35 处**引用，且要解决"哪些技能进栏、
      什么顺序"（现在由数组顺序隐式决定），**换不来任何可观察的差异**——
      对账已由 `menu_matches_the_catalogue` 钉住，有测试兜底就不值得动。
      **真正的重复清掉了一处**：`SkillDef.label` 字段与 `SkillKind::label()`
      **逐字相同**，而那个方法**没有任何调用者**——现在字段删掉、方法成为唯一出口
      （新测试 `the_display_name_has_a_single_source` 钉住）。
      **顺带核实了"攻击"那条派发规则是自洽的**（探针实测）：它 `cost = FIREBALL_COST`，
      `as_ability()` 借 `Melee` 的 id 只为走同一套校验，`can_cast` 会拿**类别 + 自己的
      cost** 判——例：2 点精力时 `Ok`、1 点时 `NotEnoughEnergy`，与火球一致，没有漏洞。
      **将来的触发条件**：菜单若真要支持"用户自己配技能栏"（拖拽 / 排序 / 多页），
      就得改成读目录 + 一份显式的"栏位布局"数据；那时 `AbilityId` 才是对的键。
- [x] **M23 意图式决策槽**（本次）：槽换形状（`Idle { intent }` / `Executing { until }`）、
      声明系统改成**填意图 + 当场物化**（**各域自己物化，不做全局派发器**）、
      `"slot_empty"` 改名 `"awaiting"`。
      执行器 / `ScheduledAction` / `ActionTiming` / 暂停机制**都不动**。
      **两个判据被拆开了**（这是本次最有价值的一处）：`is_idle()` 问"能不能占这个槽"
      （声明入口），`ready()` 问"要不要等他"（暂停断言）——旧模型只有一个
      `is_empty()` 兼两职，`Idle { intent: Some }` 这一段根本无法表达。
      `until` 的语义也统一了：**声明时** = 前摇 + 后摇（`ActionTiming::total()`），
      **执行器收尾时** = `max(后摇, 效果延迟)`（只有那时才知道要飞多久）。
      踩到的坑：`Windup` 是无时间戳的，所以测试里到处用
      `Executing { until: 0.0 }` 当"忙"——但那其实是**已经忙完**了，
      `recovery_system` 下一帧就清槽。给测试加了 `BUSY_SENTINEL = f32::INFINITY`，
      并规定生产代码里不许出现 `until: 0.0`（改用 `declared(..)`）。
      `Intent` 暂时**没有读者**（声明即物化），已在代码注释里写明原因与将来的用途。
      验收：217 测试全绿（215 单元 + 2 资产）/ clippy 零警告 / `cargo fmt --check` 通过。
- [x] **M25 前半：对抗标签闸门**（本次）：`CombatTags` 现在**跟着载荷挂到行动实体上**
      （8 个场景工厂），`interrupt_observer` 先判闸门再掷骰——
      `!interruptible || super_armor` 直接拦下，**连掷骰都不做**
      （霸体是"打断不了"，不是"比较难打断"；给玩家掷一把没用的骰子只会误导）。
      顺带修一处**文档与实现矛盾**：`COMMITTED` 的注释写着"跳跃 / 翻滚 / **招架**"，
      而招架实际标的却是 `STRIKE`——已按注释改成 `COMMITTED`。
      缺 `CombatTags` 的行动（老载荷 / 测试夹具）按普通攻击处理。
      验收：228 测试全绿（新增 2 条，且**去掉闸门后确实转红**）/ clippy 零警告 / fmt 通过。
- [x] **M25 后半：格挡**（本次）：防御链补齐第 ③ 关。`BlockChance(f32)`（单位属性，
      **来源本该是装备 / 姿态**，等 M27）+ 纯逻辑 `resolve_block` / `blocked_damage`
      （零 Bevy、零随机，骰子由应用层掷好传进来）+ `DefenseOutcome::Blocked { absorbed }`。
      **顺序按 `docs/combat.md` 第一节实现**：① 闪避 ② 招架（**拦下**，归零）
      → ③ 格挡（**按格挡率减伤**）→ ④ 护甲再减。
      **格挡是减伤不是免伤**，所以它照常触发打断（"吃到了冲击"）。
      踩到一个顺序细节：`docs` 说 ③ 在 ④ **之前**，我第一版写成了"先护甲后格挡"——
      是那条能分辨顺序的测试（`partial_blocking_stacks_with_armor_in_the_documented_order`，
      挡 50% 得 3 而不是 4）把它抓出来的。
      验收：235 测试全绿 / clippy 零警告 / fmt 通过；随机那条连跑 5 次不飘。
- [x] **M26 反应槽 + 反制（D4）**（本次）：`ReactionSlot`（挂在**被威胁的玩家**身上，
      `threat` + `suggestions` + `resolved`）取代 `ThreatWindow`；`CounterSuggestion`
      从目录算出来（遍历 `counter != None`，**无硬编码白名单**）；`CounterCost`
      进了 `AbilityDef`（`Roll = Free`、`Parry = Resource(PARRY_COST)`）；
      表态通道 = `ReactionAnswer::{Counter, Abandon}`（技能键 / 右键）。
      **顺带修掉 `opening_action` 那个死锁**：表态是显式消息，不再依赖
      "玩家那一手变了没有"（那个判据在**后摇 / 不可撤行动**期间永远为假）。
      验收：225 测试全绿 / clippy 零警告 / fmt 通过。
      **未做**：HUD 高亮"付得起"的技能（建议列表已就绪，HUD 侧未接）。
      **已补上**：技能栏现在读 `ReactionSlot.suggestions`，把能当反制的那几手高亮
      （`CounterHint::{Ready, TooExpensive}`，反制色**压过"选中"色**），
      付不起的也标出来。见 `docs/combat.md` 第四节。
      验收：`a_counter_suggestion_lights_up_the_skill_that_can_answer_it`
      （**验过不是空跑**：把建议查找改成恒返回 `None` 后转红）。
- [x] **M27 装备系统（D5）**（本次）：**设计落地为一个独立顶层域 `src/equipment/`**。
      七个文件各回答一个问题：`components`（槽位 / 物品 / 加成是什么）·
      `domain`（"基础 + 加成"怎么算，**零 Bevy**）· `relations`（`EquippedTo`）·
      `events` · `scene`（BSN 工厂）· `systems` · `plugin`。
      **六个待拍板项按设计稿的建议全部采纳**：①槽位取 `MainHand` + `OffHand` + `Armor`
      （`Trinket` 不做，触发条件见 `docs/equipment.md` 第三节）；②**基础值 + 加成**
      （设计稿候选 B）；③武器**给偏移**（第七节"乙"，且**只作用于攻击动作**——
      让一把剑改掉翻滚前摇没有道理）；④卸下的物品**落在原地、活着**；⑤⑥不做。
      **一处与设计稿的偏差**（写进了文档）：加成是**一份聚合组件** `EquipmentBonus`
      而不是"每个属性两个组件"——没有任何消费者需要"单看某一项加成"，
      拆开只会多三份写入者与三处清残留的机会。
      **接线**：`EquipmentSet` 排在 `CombatSet` **之前**（加成要在命中公式读它之前算好）；
      `spawn` 给 PC 发三格起始装备（`T` 是调试开关，不是玩法）；
      `presentation` 把**有效护甲**显示在单位面板状态行（`arm: 3`）；
      三件装备与槽位都派生了 `Reflect`，BRP 可读（`EquipmentBonus` / `EquipmentSlot` / `Item`）。
      **实机探针抓到两个单测测不到的 bug**（都记进了 `docs/equipment.md` 第七节）：
      ① 物品挂 `ChildOf(slot)` 却写了世界坐标 → 父级**再叠一次**，PC 在 `(3,-1,1)`、
      物品落在 `(6,-2,2)`；② 起始装备**每帧都发**（判据只认 `InputDriven`，而玩家身上
      没有槽位组件）→ 护甲两帧就翻倍。
      验收：305 单元 + 3 资产全绿 / clippy 零警告 / `cargo fmt --check` 通过；
      `the_equipment_armor_bonus_reaches_the_hit_formula` **验过不是空跑**
      （还原成裸 `Armor` 后转红，报掉了 15 而不是 12）、
      `the_wrong_item_is_refused_and_dropped_from_the_slot` 同（拿掉 Observer 后转红）；
      **实机**：BRP 读到三件物品 / 三个槽 / 玩家加成 `{armor:2, damage:1, windup:-0.05,
      block:0.35}`，面板 `PLAYER · arm 3`（基础 1 + 装备 2）vs `ENEMY · arm 0`，
      按 `T` 后 `arm 1`、三件物品的 `GlobalTransform` 全部等于玩家脚底 `(3,-1,1)`。
- [x] **M28 每个 mod 出 `plugin.rs`**（已完成）：三个父域
      （`combat` / `world` / `voxel_render`）的子域**都已各自出 `plugin.rs`**，
      父域只编排顺序、不注册系统。做法是每个子域声明自己的 `*Set`，
      父域一行 `.configure_sets((…).chain().in_set(XxxSet))` 串起来。
      **踩过的坑**：先按"插件添加顺序"接线时 6 条测试立刻转红（`armor_reduces_physical_damage`
      看到 100 而不是 93）——Bevy 的 `Plugin` 添加顺序**不决定**系统顺序，
      那样写出来的"顺序"是假的。子域 `plugin.rs` 数量：`combat` 7 / `world` 3 /
      `voxel_render` 2；`combat/attack` 的目录名本来就对，无需改名。
- [x] **M29 删掉 `architecture.md` / `components.md` / `NEW_DESGIN.md`**（本次）：
      替代已完成并删除三篇。删之前确认过**没有独有内容丢失**：
      `components.md` 第九节（系统 × 组件反查）与 `architecture.md` 第十一节
      （按键总表）**都已过时**——反查表里的 `ThreatWindow` / `ChildOf` 取行动者 /
      `apply_clock` 全是被取代的写法，按键表还把空格写成"暂停"（真相反查
      `src/input/keyboard.rs`，玩家可见副本是 `HELP_LINES`，两者现有测试对账）；
      `architecture.md` 第二节的「铁律 → 违反后果」表**有价值**，
      已并进 [`docs/domain.md`](docs/domain.md) 第五节。
      验收：三篇已删、全仓库无残留引用（`grep -rn 'architecture\.md\|components\.md\|NEW_DESGIN'`
      只剩本页的历史记录）、`docs/index.md` 与 `CLAUDE.md` 的文档地图已更新。
- [x] **M30 形状按需抽取（D3）—— 结论：不建 `utils` 域、不建 `Shape` 枚举。**
      形状改为**一个形状一个组件**（`HitRadius` / `MeleeShape`），判定系统紧贴各自的
      形状（`combat/targeting/`），与 `combat.md` 第三节"不要中心化类型枚举"一致。
      验收：顶层 `src/utils/` 不存在（`voxel_render/meshing/utils.rs` 是网格化内部模块，
      与本次结论无关）；`docs/domain.md` / `docs/skills.md` 已改写。
      **触发条件**：只有出现"只吃参数、不碰组件"的几何（如 `hit_test(&Shape, ..)`）
      且被两个以上域复用时，才重新考虑抽公共模块。


---

## 2026-09-25 实机核查：5 条 UI / 交互发现已修复（补记）

> 来源：另一人对界面交互做的 review（原 GitHub issue #46 / #48 / #49 / #57 / #58），
> 修复已随 PR 合入，但 **issue 当时没有关闭**——本次核对后补记于此，issue 已批量关闭。
> 核对方式：读代码 + 对**运行中的实例**用 BRP 双查（`world.get_resources` / `world.query`
> / `brp_extras_move_mouse` + `screenshot`）。

- [x] **#46 HUD 点击 / 悬停穿透：点面板会同时对世界下达指令**（提交 `bde0485`）
      **实测证据**：把光标移到技能槽中心（物理坐标 `(768,1008)`，窗口 1920×1080、
      `scale_factor 1.5`）后读到
      `PointerOverUi = true`、`HoveredCell = null`，提示条 `display: None`——
      悬停高亮与 AOE 预演**不再留在世界里**；鼠标移开又恢复拾取。
      七个区域根节点（`PlayerPanel` / `EnemyPanels` / `SkillBarPanel` / `CombatLog` /
      `ActionHint` / `HelpPanel` / `Timeline`）**都带 `FocusPolicy::Block` +
      `RelativeCursorPosition`**，`HudRoot` 保持 `Pass`。
      **实现与 issue 原方案的一处偏离（更好）**：判据不用 `Interaction`（它在按下时变
      `Pressed`、复位要等松手事件——实测踩到"卡住的 `Pressed` 让世界永远点不动"），
      改用 `RelativeCursorPosition.cursor_over`（每帧按几何重算，与点击生命周期无关），
      并且**隐藏的区域不算压在 UI 上**（否则"悬停着就被收起来"的面板会留下过期的
      `cursor_over: true` 把世界永久冻住——`F1` 帮助面板就是这个形状）。
      验收：`a_click_over_the_hud_never_becomes_a_world_command` /
      `a_declared_region_under_the_cursor_means_the_pointer_is_over_ui` /
      `a_stuck_press_does_not_keep_the_world_frozen` /
      `a_hidden_region_stops_holding_the_pointer`。

- [x] **#48 战斗日志句式拼错「敌人玩家 命中，受到 15 点伤害」**（提交 `2ba565d`）
      两个阵营名**直接相连**（`format!("{who}{text}")`，而 `text` 又以出手方开头），
      语序也反了。现在整句收成一个纯函数，语序是「出手方 → 受击方 → 数值」：
      `敌人 命中 玩家，造成 12 点伤害`；环境伤害（`source: None`）照旧只写受击方。
      **为什么四条单测没抓到**：它们只断言"两个标签都出现"，畸形句也满足。
      现在测试改成**钉整句**，并补一条
      `the_two_side_labels_are_never_glued_together`（两个标签相邻即失败）——
      两条都**验过不是空跑**（还原旧写法后同时转红）。

- [x] **#49 投射物伤害丢出手方：日志写不出「谁打的谁」**（提交 `827d596`）
      **根因不是漏写 `source`，而是"写的时候实体还活着、读的时候已经死了"**：
      `explosion.rs` 写了 `source: Some(blast.projectile)`，同一帧又把投射物 `despawn()`，
      而 `presentation/log.rs` 靠反查那个实体的 `Faction` 认出手方 → 走"无出手方"分支。
      现在来源升级成 `DamageSource { entity, faction }`——**阵营在产生伤害的那一刻就记下来**，
      `apply_damage_system` 把它透传给 `DeathEvent.killer`，日志**不再需要任何查询**。
      **顺带解掉一条隐式时序耦合**：「日志必须排在 combat 之后、且来源实体还得活着」。
      验收：`a_projectile_hit_names_the_side_that_fired` /
      `a_death_names_the_killer_even_after_the_attacker_is_gone`。

- [x] **#57 威胁格可视化**（提交 `bcffd68`）→ `src/presentation/threat_grid.rs`
      把 `Threatens.cells` 与飞行中投射物的 `TargetCell` 画在地面上（池化 8 格，
      只改位置与显隐、帧内不产生实体分配）。**玩家看到的威胁图与 AI 用的是同一份数据**
      （`docs/game-design.md`「AI 与玩家对称」那条的兑现）。
      两条纪律：**只在反应窗口开着时画**（关窗即隐藏，表现层**只读** `ReactionSlot`，
      不自己判断"还有没有威胁"，否则两个地方各有一套判据迟早分叉）；
      与鼠标悬停高亮是**两套独立视觉**（威胁贴得更低，叠一起不 z-fighting）。
      **未做**：威胁**来源圈**——它与阵营环共用零件，见 `docs/backlog/presentation.md` 的 #56。
      实机：BRP 读到池里 2 个可见、世界坐标 `(3.0,1.0)` / `(5.0,1.0)` 正是敌人火球轨迹的格中心。

- [x] **#58 时间轴悬停读数 + 战场高亮**（提交 `9937b45` / `2d3aa0d`）
      滑到时间轴色块上，状态行下方弹出一行
      `ENEMY · fireball → cell (3,1) · 0.4s · interruptible`（谁 · 什么 · 打哪 ·
      还剩多久 · 能不能被打断），同时战场上圈出那一手的主人（`TimelineFocusRing`）。
      **一行新增数据类型都没有**：那五个数本来就挂在行动实体上——做法是让**色块自己带着
      行动实体的 id**（`TimelineSlot::action`），悬停时反查（比"按车道找那个行动者的行动"稳，
      后者的判据会与分道逻辑分叉）。
      **对抗标签的读法与打断闸门共用一条口径**（`!interruptible || super_armor` →
      `super armor`）：读数说能打断、实际断不掉，比没有读数更糟，有测试钉住。
      **两处与设计稿的偏差**：读数显示的是**前摇剩余秒数**（不是前摇 / 后摇两个数，
      总时长在色块宽度里已看得见）；指示圈圈的是**行动者**而不是"威胁格"
      （格子高亮要与 `HoveredCell` 抢通道，后来由 #57 用独立的视觉层解决）。
      验收：`hovering_a_block_shows_what_that_action_is` /
      `the_readouts_countdown_follows_the_virtual_clock` /
      `the_focus_ring_circles_the_actor_of_the_hovered_action`；
      前两条**验过不是空跑**（判据从 `Hovered` 改成只认 `Pressed` 后转红）。
      ⚠️ **实机限制**：BRP 的合成光标驱动不了 `Interaction`（Bevy 每帧重算它），
      所以"鼠标真的悬停上去"这一步只能靠测试守，实机只验到接线与默认隐藏。

---

## 2026-09-26 UI / 交互第二批（10 条 backlog 条目 + 一次字体事故）

> 这一批把第一次核查留下的 P0/P1 全部落地，附带修掉两处**只有实机才看得见**的问题。
> 验收：`cargo test` **393 通过（+3 资产 = 396）/ 0 跳过** ·
> `cargo clippy --all-targets -- -D warnings` 零警告 · `cargo fmt --check` 通过 ·
> `cargo run` 无 panic、无资产错误（BRP 实测见下）。

- [x] **#47 面板 `ready` / `busy` 读反**（判据换 `is_idle()`；`DecisionSlot::ready()`
      改名 **`decided()`** 消除撞名）。
      实机：`PLAYER · ready · cell (1,0)`（槽是 `Idle { intent: null }`）与
      `ENEMY 1 · busy · cell (3,3)`（槽是 `Executing`）——**与决策槽对上了**。
      验收：`an_idle_unit_reads_ready_and_a_busy_one_reads_busy`（三个方向）。

- [x] **#50 手动暂停时状态行漏读 `ManualPause`**（HUD 侧补上；`is_frozen()` 不动）。
      验收：`the_state_line_reads_the_freeze_reasons` 从**资源**走一遍
      （旧用例直接喂 labels，验不到这个 bug）。

- [x] **#51 面板格坐标多一个前导空格**（去掉 `{:>2}`，并改掉把错误格式钉死的断言）。
      实机：`cell (1,0)` / `cell (3,3)`。

- [x] **#52 Focus 的 HUD 读数**：`FOCUS n / 3` + 三点式圆点（用掉的压暗），
      **玩家与敌人都画**（敌人也会花 Focus 闪避）。
      实机：玩家与两个敌人各一行 `FOCUS 3 / 3`。

- [x] **#53 威胁窗口两档读数**：`ReactionSlot` 加 `ThreatKind::{Incoming, InFlight}`
      （**开窗时**定下来，读数层不重新判断），`threat_hint` 纯函数产出中文两档
      （前摇给倒计时、出手给落点），**不参与 2 秒淡出**（它是持续状态），
      优先级 **被拒输入 > 威胁窗口 > 预演读数** 只写在一处。
      顺带把 `hint.rs` 拆成「取数 → 写 UI」两层。
      验收：5 条（两档文案不同、不淡出、优先级、倒计时跟虚拟时钟、友方不写成敌）。

- [x] **#54 帮助面板的 `CORE LOOP`**：世界只为谁冻结 / 时间轴怎么读 / 威胁的两条出路 /
      Focus 怎么花 / 倒计时=还能不能撤；右键补上"也是放弃反制"。
      面板下移到 6% 并加 `max_height` + `overflow: clip_y`（多 6 行后 720p 会顶到底边）。
      验收：`the_help_teaches_the_core_loop`。

- [x] **#55 技能槽点击 = 只选中**（`input` 域，12 行）：`Changed<Interaction>` → `Pressed`
      → `SelectSkill`，**不释放技能**。
      验收：`clicking_a_skill_slot_selects_it_without_casting`（断言没有
      `UseSelectedSkill` / `PlayerTakeover`；悬停不算点击）。

- [x] **#61 溢出计数 + 敌人列与战斗日志重叠**：
      `UnitPanels::dropped_enemies` + `overflow_line()`（`还有 N 个`）；
      敌人列顶上一行 `EnemyOverflow`；敌人列改用 `Column` + `justify_content: End`
      （`ColumnReverse` 会把主轴起点也翻过来，生成顺序与视觉顺序相反）；
      **`CombatLog` 的 `bottom` 不再写死 126px**，改由
      `LOG_BOTTOM_CLEARANCE = 14 + ENEMY_COLUMN_MAX_HEIGHT + 8` 算出——
      实测过一次 `Enemy2Row` 压住 `COMBAT LOG` 标题。
      验收：4 条（含"日志必须让开敌人列"这条几何不变量）。

- [x] **#62 诊断锚点补 `Reflect`**：组件补 `Reflect` + **`#[reflect(Component)]`**
      （只有 derive 不够——查询要的是那行属性带来的 `ReflectComponent`，测试当场抓到），
      资源补 `Reflect` + `#[reflect(Resource)]`；`PauseReasons` 内含 `HashSet`
      **不派生**（"别为了能看而改数据结构"），改为新增只读镜像
      **`RememberedPauseReasons`**（去重 + 排序，远程读数才可复现）。
      **实机**：`world.get_resources app::clock::RememberedPauseReasons` → `["awaiting"]`、
      `app::clock::ManualPause` → `false`（**以前两个都是 `Unknown resource type`**）。
      验收：`the_diagnostic_anchors_are_reflected`（删任意一个派生或那行属性都转红）。

- [x] **#63 MCP 输入时钟坑 + 文档漂移 + 实机核查清单**：
      `AGENTS.md` 的「测试规范」补四条（MCP 按键松手依赖时钟 → 同键第二次不触发
      `just_pressed`；`cargo run` / `cargo test` 抢 target 锁；改 UI 后要跑实机；
      诊断要读的类型必须派生 `Reflect`）；
      修掉 `docs/index.md` 的 `Empty/Windup/Recovery` 漂移与 `docs/combat.md` 的
      **死引用**（指向不存在的「`TODO.md` 的 T4」）；
      新增 [`docs/playtest-checklist.md`](docs/playtest-checklist.md)（六节 + 已知坑三条）。

- [x] **字体事故与配方固化**（这一批最有价值的一条教训）
      加了 `还有 N 个` 三个汉字后字体验收按设计转红（**它抓住了**）。重打时我
      **手抄** `tests/assets.rs` 的 `ui_text`，漏掉 `n` / `B` / `g` 等 **25 个字形**，
      而**资产测试照样绿**（它只对账中文字面量，ASCII 那一段没人查）——
      实机截图里 `awaiting` / `COMBAT` 变成了豆腐块，**只有截图能发现**。
      现在字符集由 [`tools/subset_font.py`](tools/subset_font.py)
      **从 `tests/assets.rs` 解析** + 并入全部可打印 ASCII（95 个），配方写进
      [`assets/LICENSES.md`](assets/LICENSES.md)。新字体 **134 字形 / 12.1 KB**。
      **教训**：两份真相源要一致时，让脚本去读那一份，别手抄——手抄的偏差没有测试拦得住。

---

## 2026-09-26 阵营环（#56）

- [x] **世界里的阵营标识**：单位脚下一圈贴地圆环，**蓝 = 玩家、红 = 敌人**。
      **为什么值得做**：`player.png` 与 `enemy.png` 是两张不同贴图，但 16×16 拉到
      1.8 世界单位、配上斜视角与方向光之后，实机里两个单位都读成"暗色小块 + 亮脸"——
      阵营色只存在于 HUD 与时间轴，世界里没有任何提示。而"哪个是我"是**每帧都要用的信息**。
      **实现**：`presentation/unit_sprite.rs` 出零件（`FactionRing` 标记 +
      内/外半径与抬升三个旋钮 + `faction_ring_color()`），`spawn/unit.rs` 把它作为
      单位实体的**第三个子节点**挂上（`Annulus` + 平铺旋转 + `NotShadowCaster`）。
      它是**静态**的：跟着单位的 `Transform` 走就够了，不需要系统
      （与阴影不同——阴影要贴地、要随离地高度收缩）。
      **颜色只有一份真相**：`faction_ring_color` 直接调 HUD 的 `faction_color_alpha`，
      改了 HUD 的阵营色，世界里的环跟着变。
      验收：三条新用例（组装出来的三个单位各有一个环 + 材质底色对得上阵营、
      **两个阵营不能同色**、三条几何旋钮之间的关系）。
      ⚠️ 后一条比较的是常量，用 `black_box` 把值藏起来——否则 clippy 会说
      "断言的值恒定"（**它是对的**：常量本身不是测试，"改了常量这里会红"才是）。
      **实机**：玩家脚下蓝环、两个敌人红环，都让开了阴影；BRP：
      `world.query FactionRing` → 3 个，`Transform = (0, 0.03, 0)` + 绕 X 轴 -90°。

- [x] **顺带：单位外观的三个标记补 `Reflect`**（BRP 的又一次实证）
      `FactionRing` / `UnitSprite` / `UnitShadow` 原先没派生 `Reflect`，
      `world.query FactionRing` **静默返回 0** ——而画面上明明有环。
      这正是 #62 那条"**查不到 ≠ 不存在**"：没注册的组件在远程协议里等于不存在。
      现在三个都注册了，`world.query` 一次拿到 3 个环。

---

## 2026-09-26 逻辑域与渲染分家（审计里的 A1+A2）

- [x] **把"造网格 / 材质"从逻辑域里搬出去**（纯搬家，行为一行未改）。
      **为什么**：`combat/attack/arrow.rs` / `fireball.rs` / `melee.rs` 与
      `interaction/visual.rs` 同时回答"这一手是多少数值 / 怎么飞"和"它长什么样"，
      只有后者需要渲染类型；连带后果是**逻辑域的插件里得装资产管线**
      （`init_asset::<Mesh>()` / `ScenePlugin`），否则逻辑域的单测跑不起来。
      **改法**：照 `equipment/` 已有的形状开两个新文件——
      [`src/combat/attack/scene.rs`](src/combat/attack/scene.rs)（`arrow_scene` /
      `melee_scene` / `fireball_scene`）与
      [`src/interaction/scene.rs`](src/interaction/scene.rs)（高亮 + 两个预演指示器
      + 全部颜色与几何常量）。
      **边界（实测出来的，比审计原文更准）**：判断一个工厂该不该搬，看它**有没有渲染类型**。
      `shoot_action_scene` / `melee_action_scene` 只挂载荷与节奏、没有任何 `Mesh3d`，
      **留在原地**；`interaction/visual.rs` 仍会碰 `MeshMaterial3d<StandardMaterial>`，
      因为它要改的是**一个具体网格实例的颜色**，那是它的本职。
      **量化验收**：
      - `combat` 只剩 `scene.rs` 含渲染类型；`movement` / `timeline` / `ai` / `input` **归零**；
      - **四个逻辑域的 `plugin.rs` 里 `init_asset` / `ScenePlugin` 全部归零**——这才是收益；
      - `cargo test` 395 通过 / clippy 零警告 / fmt 通过。
      ⚠️ **搬迁时出过一次事故，值得记**：用 `Get-Content | Set-Content` 同一个文件砍行时，
      输入流锁住了文件；第二次尝试又因为 `$body` 取空，**把 `interaction/visual.rs`
      覆盖成只剩文件头**。靠 `git checkout --` 恢复 HEAD 再重做才没丢东西。
      **教训**：大段删改不要用 shell 拼接——用编辑工具，或先写临时文件再整体替换，
      并且**动完立刻 `cargo check`**。

---

## 2026-09-26 更正一条旧结论：CJK 断行的 `ICU4X` 告警**确实会出现**

- [x] **恢复 `icu_provider=error` 静音，并把根因写清楚。**
      **怎么发现的**：本轮给提示条加了中文威胁读数（#53）之后，实机控制台开始刷
      `ICU4X data error: No segmentation model for complex script: Chinese/Japanese`。
      **根因（读依赖源码确证，不是猜）**：
      - parley 0.9 构造行分段器只走 `LineSegmenter::new_for_non_complex_scripts`
        （`parley-0.9.0/src/analysis/mod.rs:56/63/70`）；
      - 那个构造函数把 `complex` 载荷设成**空的**（`ComplexPayloadsBorrowed::new()`
        → `icu_segmenter-2.3.0/src/line.rs:462`）；
      - 断行遇到 CJK 时会问 `ComplexScript::ChineseOrJapanese` 那一支
        （`complex/mod.rs:206`），拿不到 `ja` 模型 → `DataError::custom(..)`
        **无条件打一行 error**，然后退回按字断行。
      - 旁证：`cargo tree -e features -i icu_segmenter` 显示只启用了 `compiled_data`，
        **没有 `auto`**——所以 `ja` 模型确实不在编译进去的数据里。
      **结论修正**：2026-09 那条「实测复现不出来 / 报错分支根本不会被走到」**是错的**
      （当时大概没让中文真的走到**断行**这条路径）。正确表述是：
      **功能是对的（中文照常换行），只是每次断行刷一行日志。**
      **处置**：`src/main.rs` 的 `LogPlugin` 过滤器恢复 `icu_provider=error`，
      并在注释里写明上面这条链路与"为什么不开 `auto` 特性"（为一条日志拉一套模型数据
      不划算；真要中文分词质量时再评估）。

---

## 2026-09-27 实机复跑：修掉一处**只有截图能发现**的豆腐块

- [x] **威胁读数整行是豆腐块**（fatal，已修）
      **怎么发现的**：给威胁来源圈做实机验证时，截图里那行读数是
      `敌 fireball □□□□ (0.3s □□□□) · □□□ (E □□□ / □□□)`——
      中文全成了豆腐块。**BRP 读到的是完整正确的字符串**
      （`敌 fireball 锁定你（0.3s 后落地） · 可打断（E 翻滚躲 / 右键忍）`），
      所以这条 bug **只有截图能发现**：文字内容对、字形缺失。
      **根因**：字体验收的"真相源"列表 `chinese_in_source()` 只列了四个文件
      （`log.rs` / 面板模型 / 时间轴模型 / 方块交互），**漏了 `hud/hint.rs`**。
      于是 #53 在那里新增的 20 个汉字（锁/定/你/可/打/断/翻/滚/躲/右/键/忍/友/方/
      后/落/地/已/出/手）**从来没有被要求进字体**，而那条验收测试一直绿着——
      它只能查"字在不在字体里"，查不出"这个文件压根没被抽"。
      **改法**：① 把 `src/presentation/hud/hint.rs` 加进 `chinese_in_source()`
      与 `tools/subset_font.py` 的 `SOURCES`（两处必须同步，已在两处都写了注释）；
      ② 重打子集，**154 字形 / 16 KB**（原 134 / 12 KB）；
      ③ 新增 `every_chinese_source_is_registered_in_the_font_charset`：
      用**只可能出现在某个产处**的字做哨兵（`锁/忍/滚/落` → hint、`命/杀/阵/亡` → log、
      `还/个` → 面板），谁把产处移出列表、或新增中文文案却忘了登记，都会立刻红。
      **实机复验**：同一场景重跑，读数完整显示
      `敌 fireball 锁定你（0.3s 后落地） · 可打断（E 翻滚躲 / 右键忍）`，无豆腐块。
      **教训**："两份真相源要一致"这类漏洞，测试只能守住比对得到的部分——
      **列表本身漏项，是没有任何测试能自动发现的**，除非再加一层"列表覆盖"的哨兵。

### 同一次复跑抓到的第二个 bug：三行敌人面板互相压叠

- [x] **敌人面板行高不够 → 多行压叠**（fatal，已修）
      **现象**：`ENEMY_SPAWNS` 临时加到 4 个后截图，`ENEMY 2` 与 `ENEMY 3` 的文字
      **叠在一起**（第二行的状态行压在第三行的头像上），列底部还有被裁掉的残字。
      **量到的真数据**：`Enemy1Row` = **225px**（玩家面板只有 156px），
      整列 `EnemyPanels` = **722px**，而我给的 `ENEMY_COLUMN_MAX_HEIGHT` 只有 491px
      —— 列装不下 → 行被压叠。`ENEMY_ROW_MIN_HEIGHT = 96` 这个下界从来没起过作用。
      **根因**：敌人的状态行**换行了**（`Enemy1StateLine` = 47px，即两行）。
      它带了 `ammo 3/3 · arm 0`——这两项对敌人**恒为 3/3 与 0**，
      读它得不到任何信息，却把 480px 宽的行撑爆。
      **改法**：`UnitRow::state_line` 拆出 `state_line_with(…, show_resources)`，
      由 `UnitPanels::state_line(slot)` 按**阵营**（不是按"值是多少"猜）决定：
      玩家保留弹药与护甲（装备一穿一脱 `arm` 立刻变），敌人不带。
      状态行回到单行 → 行高降到玩家面板量级 → 三行各自独立。
      **顺带校准**：`ENEMY_ROW_MIN_HEIGHT` 96 → 150（并写清"必须 ≥ 实测行高，
      给少了不是挤一点而是压叠"）；`ENEMY_OVERFLOW_HEIGHT` 24 → 23。
      **验收**：`the_enemy_state_line_leaves_out_the_resources_it_cannot_use`
      （敌人行无 `ammo`/`arm`，玩家行保留）+ 4 敌人实机截图：
      三行独立、状态行单行、`还有 1 个` 正常显示、日志在列上方不重叠。
      **教训**：`ENEMY_ROW_MIN_HEIGHT` 那类"下限"常量，**如果比实测内容小，
      它不是无害的下限，而是被当成硬上限导致压叠**——量一次真数据比猜一个数便宜。

---

## 2026-09-27 实机复跑第 4 节（冻结的三种原因），顺带修掉诊断快照的两个缺陷

- [x] **`manual` 状态行第一次有画面证据**（#50 的真正收尾）
      四种情形全部走通：`FROZEN · awaiting` / `FROZEN · threat` /
      **`FROZEN · manual`** / **`FROZEN · awaiting + manual`**（字母序拼接正确）。
      **手法**（可复现，也都写进了 [`playtest-checklist.md`](playtest-checklist.md) 第 4 节）：
      要看到 `manual` **单独**出现，必须让"世界本该在跑"——玩家空闲时按 `P` 是看不到的，
      因为清空原因后 `awaiting` 下一帧立刻断言回来（设计如此），`ManualPause` 从未置位。
      做法：把玩家 `DecisionSlot` 改成 `{"Executing":{"until":99}}`（忙碌 → 不再断言
      `awaiting`），再把窗口 `ReactionSlot.resolved` 置 `true` 让在飞的投射物落地，
      世界恢复 `RUNNING`，此时按 `P` → `FROZEN · manual`。
      ⚠️ 途中踩到：删掉敌人**不会**关威胁窗口——`ReactionSlot` 用**它自己的**
      `threat` 实体引用，在飞的投射物仍瞄着玩家。

- [x] **诊断快照 `PauseLabels`（原 `RememberedPauseReasons`）有两个缺陷，已修**
      **症状**：实机排查时读它得到 `["awaiting", "threat"]`，而当时玩家明明在
      `Executing`（`awaiting` 根本没有断言）、状态行也只写 `threat`——**它在说谎**。
      **缺陷① 它是历史并集，不是当帧快照**：`remember()` 只追加、从不清空，
      于是它记的是"这辈子出现过哪些原因"。对"**此刻**为什么冻着"这个问题，
      并集给的是**错答案**——我据此把排查方向整个带偏（去追一个不存在的 `awaiting`）。
      更糟的是它的访问器文档写着"**这一帧**的原因表里有哪些"，**行为与自己的契约相反**。
      原来的理由写在类型文档里（"与'这一刻是哪个'相比是超集，信息只多不少"）——
      **这个理由不成立**：超集只对"出现过没有"是超集，对"现在是哪个"是**错误值**。
      诊断值必须能被当成"现状"读；要问"某个窗口开过没有"，去读窗口的 `resolved`
      或当场采样。
      **缺陷② 它永远不含 `manual`**：只喂 `reasons.labels()`，而手动暂停按设计
      **不在** `PauseReasons` 里——于是玩家自己按的那一下在诊断里**完全看不见**，
      连"超集"都算不上。
      **改法**：① `remember()` → `rebuild(&reasons, manual)`，**每帧整体重建**
      （清空 → 填 → 去重 → 排序）；② 把 `manual` 并进去，且 `MANUAL_LABEL` 常量
      **下沉到 `clock`**（手动暂停是时钟状态，不是显示层文案）——
      `presentation::hud::timeline::model` 改为 `pub use crate::clock::MANUAL_LABEL`，
      于是"状态行显示的那一串"与"远程读到的那一串"**同源**，不可能各写一份而漂移；
      ③ 改名 `RememberedPauseReasons` → **`PauseLabels`**（旧名既暗示记住历史、
      访问器文档又说"这一帧"，名字与行为互相矛盾）。
      **验收**：新增 3 条测试钉住新语义——
      `the_snapshot_is_this_frame_not_everything_ever_seen`（上一帧的原因必须消失、
      没人断言时清空）、`the_snapshot_carries_the_manual_pause`、
      `the_snapshot_joins_reasons_and_manual_in_alphabetical_order`。
      **教训**：一个只会追加的诊断值，看起来"信息只多不少"，实际是**把现状问题
      答成历史问题**。这类"名字/文档说 A、实现做 B"的偏差，只有拿它当真去排查时
      才会暴露——和豆腐块一样，属于**测试全绿而读数在说谎**。
      **顺带修掉一处排序不一致**：`freeze_labels` 原先把 `manual` **贴在末尾**、
      事后不排序，于是状态行实测出现 `threat + manual`——而本清单的验收写的正是
      "**按字母序**拼出来"（`awaiting + manual` 只是碰巧合序）。现在全局排序，
      状态行与 `PauseLabels` **逐字相同**（实测
      `["awaiting","manual"]` ↔ `FROZEN · awaiting + manual`），
      "屏幕上写的"与"远程读到的"可以直接对账。

---

## 2026-09-27 实机复跑第 1 节（键盘）：等待动作在面板上没有名字

- [x] **`Space`（等待）的面板读数退化成 `act: action`**（已修）
      **怎么发现的**：按空格后玩家槽确实变 `Executing`（等待生效、世界继续跑），
      但面板那一行是 `act: action`——`payload_name()` 给
      `move` / `jump` / `roll` / `parry` / `fireball` / `shoot` / `melee` 都取了名，
      **唯独没有等待的分支**，于是落到兜底的 `"action"`。玩家按了空格，
      屏幕等于什么都没说。
      **改法**：`payload_name` 增加 `WaitAction` 分支 → `"wait"`；
      `PayloadQueries` 加 `waits` 字段（`SystemParam` 派生，调用方无感）；
      `action_text` 与 `update_action_labels_system` 各加一个查询参数。
      **验收**：`the_wait_action_has_a_name_of_its_own`。
      **教训**：兜底分支（`"action"`）会**静默吞掉**"新载荷忘了登记"这种事——
      屏幕上不报错、只显得含糊。它的注释现在写明"兜底只该是临时状态，不该是常态"。

### 同一次复跑确认无误的几条（顺带把可复现手法记下来）

- **方向键走一格、停在格心**：`Cell (1,0) → (1,-1)`，`Transform (3.0,-1.0,-1.0)`
  正是格心。屏幕→世界：`+x → −z`、`+y → −x`。
  顺带确认**按住不重复**是设计（`last_axis`：一次按下 = 一次决策，松开才复位）。
- **`X` + 方向 = 跨两格**：`(1,-1) → (-1,-1)`，正好 2 格。
- **`Q` = 火球**：`ammo 3/3 → 1/3`——`config/actions.ron` 的 `fireball.cost = 2`，
  所以**一次按键花 2 发是对的**（先怀疑过双触发，查配置排除了）；
  日志 `[2.6s] 玩家 命中 敌人，造成 13 点伤害`（`power 12` + 装备加成）。
- **`W` = 横扫**：面板 `act: melee (windup 0.2s)`。
- **`T` = 穿脱装备**：面板 `arm 3 → 1`，读数跟着装备走。
- **复跑手法（可复用）**：要稳定读到"这一手叫什么"，**先把世界冻住**
  （`world.insert_resources app::clock::ManualPause = true`）再按键——
  前摇不会流逝，面板的 `act:` 就一直停在那里。直接按会在一两百毫秒内结算完，
  读到的只剩 `act: -`；等待只有 1 秒，不冻住必然错过。

---

## 2026-09-27 实机复跑第 2 节（鼠标），并修掉一处诊断反射缺口

### 验完的四条

- **左键点空地走位**：`HoveredCell = (4,2)` → 玩家 `Cell (1,0) → (4,2)`，
  `Transform (9.0,-1.0,5.0)` 正是格心（跨 3 格的多格移动也走到了）。
  顺带确认**合成光标能驱动世界拾取**——与文末"驱动不了 `Interaction`"那条坑不矛盾：
  拾取读窗口光标位置，`Interaction` 是 Bevy 每帧重算的另一条路。
- **右键 = 放弃反制**（威胁窗口里）：`ReactionSlot.resolved: false → true`，
  `PauseLabels` 从 `["manual","threat"]` 变 `["manual"]`——威胁那条原因正确消失。
- **中键拖拽平移**：相机 `(15,14,13) → (27.83,14.0,12.83)`，高度 `y` **精确保持 14.0**。
- **滚轮缩放**：相机 `(27.83,14.0,12.83) → (38.63,26.6,23.63)`，沿视线后退，
  而 `Projection.fov` **不变**（缩放靠移动相机，不改视场角）。

### 抓到一个新 bug：敌人面板的 `act:` 行「按阵营」而不是「按行」

**证据**：场上只有 **2** 个敌人，而第三个（空）行的 `ActionLabel` 也在显示
`act: fireball (windup 0.3s)`；四个 `ActionLabel` 实体（1 玩家 + 3 敌人）里
三个敌人标签文本**逐字相同**。
**根因**：`panels/scene.rs` 挂的是 `ActionLabel { faction }`（只带阵营、不带行号），
`update_action_labels_system` 又按 `slot(faction)` 写单格快照——而 HP / EN / Focus /
Insight 那些读数**都是按 `PanelSlot` 分行的**，这是面板里唯一一处行列不对应。
**为什么现在才暴露**：两个敌人常做同样的事（同一套 AI、同样的距离），那时看不出来。
**已记档**（[`docs/backlog/hud.md`](docs/backlog/hud.md) 的 P2 bug 条），
含改法：把动作文案并进面板模型、与其它读数同源，`ActionLabel` 与
`update_action_labels_system` 整体删掉；**别**在系统里按距离重排一遍敌人
（那会把"离玩家最近的排序"复制成两份）。

### 修掉一处诊断反射缺口（今天第三次踩同一个坑）

**`ActionOf` 与 `ScheduledAction` 都没有派生 `Reflect`**，于是 BRP 的
`world.query app::timeline::ownership::ActionOf` **静默返回 0**——排查时看着像
"没有行动实体"（实际敌人正挂着火球前摇）。`ActionOf` 是"哪条行动是谁的"的**唯一**入口，
`ScheduledAction` 是"这一手什么时候落地"的唯一入口，二者都该可远程读。
**改法**：两个类型都补 `Reflect` + `#[reflect(Component)]`，
并加进 `the_diagnostic_anchors_are_reflected` 的锚点清单（丢了派生会立刻转红）。
**教训**：这个坑 2026-09-27 一天之内踩了三次（`Visibility` 的类型路径、
`ActionOf` 的类型路径、`ActionOf` 没注册）——**排查"某类实体不存在"之前，
先确认那个类型在反射表里**。
