# Project Timeless — 进度与 backlog

> 本文件是**唯一的进度真相**：里程碑勾选状态、待办清单、依赖索引。
> 设计文档只写「是什么、为什么」，见 [`docs/index.md`](docs/index.md)。
> 勾选规则：必须有验收证据（命令输出 / 测试名 / 控制台片段）才允许 `[x]`。

## 验收命令（仓库根目录）

```bash
cargo test                                  # 178 通过（176 单元 + 2 资产验收）/ 0 跳过
cargo clippy --all-targets -- -D warnings   # 零警告
cargo fmt --check
cargo run                                   # 冒烟：体素地形 + 世界空间战斗
```

实机冒烟清单（每轮提交前）：启动无 panic · 无资产加载错误 ·
决策 → 声明 → 前摇到点落地 → 扣血 → `F5` 重置可复现。

## 当前代码是什么

`src/`（package `app`）= 10 个领域 + 组装车间：

- `world` 32³ 体素区块 / 噪声地形 / 体素读写（零渲染依赖，`MinimalPlugins` 可单测）
- `voxel_render` 异步面剔除网格化 / 按类型分组材质 / 面朝向明暗
- `movement` `Cell` + `MoveGoal` 格子决策、`Transform` + `Velocity` 连续位移、
  移动 / 跳跃 / 翻滚载荷与执行器
- `combat` 生命 / 护甲公式 / 碰撞与近战扇形 / 攻击实体生命周期 / 箭矢与横扫 /
  火球锁格 + 真实距离 AoE / 精力 / 翻滚无敌帧 / 招架反制 / 威胁检测（反应系统）/
  `SKILLS` 注册表与技能菜单
- `timeline` **无回合**调度：`DecisionSlot` 三态（`Empty` / `Windup` /
  `Recovery { until }`）直接写在行动者身上，`ScheduledAction` 的时间戳回答"到点了没有"；
  暂停是**每帧断言**——谁这一帧还想停表就写一条 `PauseRequest::Pause(原因)`，
  `process_pause_requests` 每帧重建 `PauseReasons`，唯一的时钟写入点是帧末的 `apply_clock`；
  行动归行动者所有（`ActionOf` / `Actions`，人没了行动跟着没），`Focus` 让玩家把一次前摇买掉
- `ai` 六种意图（含威胁预判）+ 声明行动
- `input` 只翻译（含 `F5` → `ResetBattle`、空格 → `PauseRequest`、`PlayerTakeover`）·
  `interaction` 鼠标拾取 / 高亮 / 预演 / 点击解释
- `presentation` 相机 / 单位纸片与贴地阴影 / 装饰 / 中文日志 / 英文 HUD
- `spawn` 组装车间（消费 `ResetBattle`，不认识按键）

域地图与跨域契约见 [`docs/domain.md`](docs/domain.md)，文档入口是
[`docs/index.md`](docs/index.md)（`architecture.md` / `components.md` 正在被取代，只作历史参考）。

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
      `MELEE_TIMING` / `ARROW_TIMING` → `combat/skills/actions.rs`、
      `FIREBALL_TIMING` → `combat/skills/fireball.rs`、`PARRY_TIMING` →
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
      各写一遍（其中 8 个是活路径、1 个是未注册的 `declare_skill_system`），现在收成一处。
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
| — | 文档 | **重建**（不增补）：新增 domain / relations / combat / skills / equipment，`architecture.md` 与 `components.md` 待替代完成后删除 |

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
| D2 | 技能 | **`skills` 抽成顶层域**（不再挂在 `combat` 下）；**移动 / 跳跃 / 翻滚也是技能**，不做特殊处理；`AbilityDef` 是**静态、可序列化**的那一半（不许出现 `Entity` / 闭包），各机制域通过 `RegisterAbility` 把自己的定义交上来；`combat/skills` 改名 **`combat/attack`** |
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
- [ ] **等待时长要可配**：现在是 `timeline::WAIT_SECONDS = 1.0` 常量；
      等动作数值外置（`.ron`）之后应进技能表。
- [ ] **暂停：情形 A 已由「等待」动作解决**（`feat/wait-action`）：玩家空闲时按空格
      生成一条占槽 1s 的等待行动 → `awaiting` 消失 → 世界继续跑。
      **`ManualPause` 因此只服务"忙碌 + 运行"那一种情形**（他没有槽可占）。
- [ ] **威胁：按 action 上报一次**：取代现在靠 `ThreatWindow.dismissed` 的全局记账，
      语义变成"威胁是 action 的属性"，于是"放开之后第二帧又冻住"必然意味着**新威胁**。
      落在 M26 一起做。
- [ ] **M24c 菜单改读目录**：`SKILLS`（4 项，其中"攻击"是**派发规则**不是技能）与目录
      （7 项）不是一一对应，迁移要先定 `MenuSelection` 存什么（`SkillKind` 还是 `AbilityId`）。
      现状已由 `menu_matches_the_catalogue` 兜住，不急。
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
- [ ] **M25 对抗标签 + 格挡**：`CombatTags` 闸门（含霸体）+ 防御链补格挡
      （格挡率来自装备 / 姿态 `BlockChance`）；**保留**掷骰对抗 `interrupt_lands`。
- [x] **M26 反应槽 + 反制（D4）**（本次）：`ReactionSlot`（挂在**被威胁的玩家**身上，
      `threat` + `suggestions` + `resolved`）取代 `ThreatWindow`；`CounterSuggestion`
      从目录算出来（遍历 `counter != None`，**无硬编码白名单**）；`CounterCost`
      进了 `AbilityDef`（`Roll = Free`、`Parry = Resource(PARRY_COST)`）；
      表态通道 = `ReactionAnswer::{Counter, Abandon}`（技能键 / 右键）。
      **顺带修掉 `opening_action` 那个死锁**：表态是显式消息，不再依赖
      "玩家那一手变了没有"（那个判据在**后摇 / 不可撤行动**期间永远为假）。
      验收：225 测试全绿 / clippy 零警告 / fmt 通过。
      **未做**：HUD 高亮"付得起"的技能（建议列表已就绪，HUD 侧未接）。
- [ ] **M27 装备系统（D5）**：全新（槽位 / `EquippedTo` / 类型校验 Observer / 穿脱），
      并定下**属性的「基础值 + 加成」结构**。
- [ ] **M28 每个 mod 出 `plugin.rs`**：`CombatPlugin` 只编排子域顺序、不注册系统；
      顺带把 `combat/skills` 改名 `combat/attack`（D2 的收尾）。与其它步独立，随时可做。
- [ ] **M29 删掉 `architecture.md` / `components.md` / `NEW_DESGIN.md`**：替代完成后的收尾。
- [x] **M30 形状按需抽取（D3）—— 结论：不建 `utils` 域、不建 `Shape` 枚举。**
      形状改为**一个形状一个组件**（`HitRadius` / `MeleeShape`），判定系统紧贴各自的
      形状（`combat/targeting/`），与 `combat.md` 第三节"不要中心化类型枚举"一致。
      验收：顶层 `src/utils/` 不存在（`voxel_render/meshing/utils.rs` 是网格化内部模块，
      与本次结论无关）；`docs/domain.md` / `docs/skills.md` 已改写。
      **触发条件**：只有出现"只吃参数、不碰组件"的几何（如 `hit_test(&Shape, ..)`）
      且被两个以上域复用时，才重新考虑抽公共模块。

## 待办

### 工程债（按优先级）

- [ ] **动作数值外置**：各领域的 `*_TIMING` 与 `SKILLS` 的数值改成 `.ron`
      （serde + ron），应用层不再硬编码技能数值；顺带做 `ActionRegistry` 资源，
      让 HUD / 菜单从注册表读选项与消耗。
- [x] **死代码清理**：删除 `defense/actions.rs` 里重复且未注册的
      `expire_defense_markers_system`、`lifecycle::manage_projectile_hits_system`、
      写而无消费的 `AttackResolved`。剩下的 `declare_skill_system` / `arrow_scene`
      是「单体狙击」的预留实现（收尾已与火球对齐），接输入即可用。
- [x] **`Space` 手动暂停只前进一帧**：改成暂停原因集合 + **每帧断言**，手动暂停一直有效
      直到再按一次；"按一下是暂停还是恢复"的闩住在
      `input::keyboard::pause_input_system`（读 `PauseReasons`）。
- [x] **箭矢的收尾方式**：`shoot_action_executor_system` 现在忙到
      `距离 / ARROW_SPEED`（与火球共用 `DecisionSlot::recovering(.., effect_delay)`）。
- [x] **玩家身份收口**：新增 `timeline::InputDriven` 标记（组装层挂在玩家身上），
      时间线 / 反应系统 / 撤销 / 各玩家声明系统都认它，不再满世界找 `Faction::Player`。
      表现层仍按 `Faction` 找单位——它看的是阵营，不是输入归属。
- [ ] **反应窗口的粒度**：现在是"一次威胁一个窗口"，多段攻击（连续三刀）只会问玩家一次。
      将来按威胁**来源**分别开窗（`ThreatWindow` 存集合而不是一个 `Option`）。
- [ ] **威胁窗口存的是行动者而不是行动**：`ThreatWindow.opening_action` 存的是玩家实体，`detect_threat_system` 用它判断"表态了没有"。玩家在**前摇中**被威胁冻结时，撤销再声明一手不会改变这个值，窗口于是永远等不到表态——双方一起冻死。改成存行动实体即可修好（行动实体在撤销/重新声明时会变）。
- [ ] **AI 不会用 Focus**：`Focus` 只在玩家侧，敌人声明固定排前摇。
      要让精英怪也会抢先手，得给 AI 一套"什么时候值得花资源"的策略。
- [ ] **`AttackFrame` 没有消费者**：攻击实体都挂着它，但没有系统读——
      它是「洞察力」面板的读数（"谁先动"），接上或删掉。
- [ ] **没有生产者的预留类型**：`Voxel` / `VoxelPos` / `ChunkPinned`——接上或删掉。
- [ ] **开发热重载** ⛔ **环境阻塞**：`bevy/file_watcher` 要 `notify-debouncer-full = 0.7.0`，
      而本机用的清华镜像只到 **0.6.0**，`cargo build` 直接解析失败（实测）。
      解法只有两条：换一个能拿到 0.7.0 的源，或等镜像同步。
      `Cargo.toml` 里留了注释说明加哪个 feature。

### 玩法与表现

- [ ] **箭矢接回输入**：`ShootAction` / `arrow_scene` 已实现且被测试覆盖，
      但 `declare_skill_system` 未注册（避免与火球抢同一条 `FireCommand`），
      计划作为「单体狙击」技能接回。
- [ ] **单位贴地与体素碰撞**：`movement` 查询 `world` 的体素决定目标格是否可走
      （当前点击目标格走直线，没有可行走性判定与寻路）。
- [ ] **火球 / 命中特效**：目前只有实体本身，没有粒子或 Gizmos。
- [ ] **护甲接进组装层**：公式支持 `Armor`，但还没有任何单位挂它。
- [ ] **接入 `assets/textures/ground/grass.png`**（当前无代码引用）。

### 渲染优化（`voxel_render`）

- [ ] **贪婪网格化**：同材质共面合并成矩形，替代逐面四边形。
- [ ] **纹理图集 / UV**：`materials/assets.rs` 换图集，网格化代码不动。
- [ ] **AO**：`lighting` 从面朝向明暗升级为按顶点的邻域遮挡。
- [ ] **区块持久化**：只保存被改动的区块（`ChunkPinned` + 存档）。
- [ ] **方块交互**：放置 / 破坏走 `world::storage::set_voxel`，自动触发重建网格。

### 字体与文本

- [ ] **字体覆盖验收**：`tests/assets.rs` 目前只验「文件在、是合法 sfnt」；
      逐字查 `cmap` 要引第三方库（`skrifa`，已移除）。release 前评估是否加回。
- [ ] **CJK 断行**：Bevy 文本栈缺 `icu_segmenter` 的 CJK 分词模型，运行时会打印
      `ICU4X data error: No segmentation model for complex script`
      （正文仍正常渲染，只是断行退化）。
- [ ] **中文 HUD 文案**：字体已就位，把 HUD 文案翻成中文还需要中文排版
      （断行 / 标点挤压）。
- [ ] **字体体积**：现为 8.3 MB 全覆盖，可子集化到几十 KB。

### 策略深度（目标形态，未落地）

- [ ] **多敌人战斗**：单例查询改多实体查询，AI 每单位独立意图与威胁排序。
- [ ] **资源分线**：`AmmoPouch`（重击 / 射击）/ 架势槽 `Poise`（打断抗性 / 格挡）；
      平 A 免费。
- [ ] **格挡减伤**：与现有翻滚 / 招架并列的第三条防御路径。
- [ ] **范围攻击排程 / 冲刺（位移 2 格）**。
- [ ] **`ActionTemplate` 资产图**：动作的静态定义（相位 / 消耗 / 效果 / 可取消规则）
      资产化，按边条件在图上转移。当前已落地的最小形态是
      `ActionTiming { windup, recovery }` + 行动实体。
- [ ] **延迟命中实体化（`PendingHit`）**：把延迟 AOE / 地面效果排到未来事件堆上。

### 信息层（G 层：信息即力量）

- [ ] **洞察力**：查看怪物数据（帧 / 射程 / 打断抗性 / 血量）、帧窗口细节。
- [ ] **战斗日志回看**（历史 N 条）与**死亡复盘**（谁在哪个时刻命中了谁）。
- [ ] 成长以知识为主：升级解锁信息权限而非纯数值。

## 依赖与文档索引

> 本机 crates.io 直连不可用，版本经**清华镜像稀疏索引**查询确认。
> 依赖一律手动写入 `Cargo.toml`（`cargo add` 在镜像下不可用）；
> 新依赖确认版本后先登记下表再引入。

| 依赖 | 版本 | 用途 |
| :--- | :--- | :--- |
| bevy | 0.19.1 | 引擎（`Cargo.toml` 写 `0.19`，`Cargo.lock` 锁 0.19.1） |
| rand | 0.10.2 | 装饰物随机摆放 |
| bevy_brp_extras | 0.22 | 运行时调试协议扩展：截图 / 输入模拟 / 干净退出 |
| serde + ron | 未引入 | 配置序列化（「动作数值外置」时引入） |

引擎官方文档：<https://bevy.org/learn/> ·
迁移指南：<https://bevy.org/learn/migration-guides/> ·
符号检索：<https://docs.rs/bevy/latest/bevy/?search=>

## 开发规范（摘要）

1. **分层解耦**：`combat/formula/domain.rs` 零 Bevy 依赖；应用层不含伤害公式。
2. **数据驱动**：数值 / 技能走外部配置（见「动作数值外置」），应用层不硬编码。
3. **消息通信**：模块间用 Bevy `Message`；组件 / 消息 / 系统同属一个领域文件；
   UI 输入只翻译成消息；消息在**消费方**插件注册并注明谁写谁消费。
4. **文档先行**：用 Bevy API 前先查 docs.rs / 官方示例（`bevy-019-docs` skill），
   不依赖训练记忆。
5. **文档防漂移**：进度只写本文件；文档里引用的类型名必须先在 `src/` 里 grep 确认存在。

完整规范见 [`AGENTS.md`](AGENTS.md)。
