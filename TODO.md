# Project Timeless — 进度与 backlog

> 本文件是**唯一的进度真相**：里程碑勾选状态、待办清单、依赖索引。
> 设计文档只写「是什么、为什么」，见 [`docs/index.md`](docs/index.md)。
> 勾选规则：必须有验收证据（命令输出 / 测试名 / 控制台片段）才允许 `[x]`。

## 验收命令（仓库根目录）

```bash
cargo test                                  # 358 通过（355 单元 + 3 资产验收）/ 0 跳过
cargo clippy --all-targets -- -D warnings   # 零警告
cargo fmt --check
cargo run                                   # 冒烟：体素地形 + 世界空间战斗
```

实机冒烟清单（每轮提交前）：启动无 panic · 无资产加载错误 ·
决策 → 声明 → 前摇到点落地 → 扣血 → `F5` 重置可复现。

## 当前代码是什么

`src/`（package `app`）= 12 个领域 + 组装车间：

- `world` 32³ 体素区块 / 噪声地形 / 体素读写 / 方块交互（零渲染依赖，`MinimalPlugins` 可单测）
- `voxel_render` 异步面剔除网格化 / 按类型分组材质 / 面朝向明暗
- `movement` `Cell` + `MoveGoal` 格子决策、`Transform` + `Velocity` 连续位移、
  移动 / 跳跃 / 翻滚载荷与执行器
- `combat` 生命 / 护甲公式 / 碰撞与近战扇形 / 攻击实体生命周期 / 箭矢与横扫 /
  火球锁格 + 真实距离 AoE / 精力 / 翻滚无敌帧 / 招架反制 / 格挡 / 反应槽
  （8 个子域，**每个子域一个 `plugin.rs`**，`CombatPlugin` 只编排顺序）
- `skills` **静态目录**（顶层域）：`AbilityId` / `AbilityDef` / `SkillRegistry` /
  `RegisterAbility` / `can_cast`；数值仍归各机制域，目录只聚合
- `equipment` **装备**（顶层域）：槽位（`ChildOf` PC）/ 物品（`EquippedTo` 槽位）/
  类型校验 Observer / **「基础值 + 加成」**——基础值由组装层写、加成只有它写，
  有效值走纯函数（`armor_of` / `weapon_damage` / `weapon_timing`）
- `timeline` **无回合**调度：`DecisionSlot` 两态（`Idle { intent }` / `Executing { until }`，
  见 M23）写在行动者身上；`is_idle()` 问"能不能占槽"、`ready()` 问"要不要等他"；
  行动归行动者所有（`ActionOf` / `Actions`，人没了行动跟着没），`Focus`
  （**每单位一份的组件**）让玩家与敌人都能把一次前摇买掉；
  另有「等待」动作（`wait.rs`，玩家空格绑定的"让我想想"）
- `clock` **通用冻结设施**（不属于任何领域）：`PauseRequest` 每帧断言 +
  `ManualPause` 布尔，`process_pause_requests` 是**唯一的 `Time<Virtual>` 写入点**。
  谁拥有事实谁断言——`timeline` 说 `awaiting`、`combat::reaction` 说 `threat`、
  `input` 只发消息
- `ai` 六种战术（含威胁预判与花钱买前摇的闪避）+ 声明行动
- `input` 只翻译（`F5` → `ResetBattle`、**空格 → 等待**、`P` → 手动暂停、`B`/`V` 放/挖方块、
  `T` → 穿脱装备、`PlayerTakeover`）· `interaction` 鼠标拾取 / 高亮 / 预演 / 点击解释
- `presentation` 相机 / 单位纸片与贴地阴影 / 装饰 / 中文日志 / 英文 HUD
- `spawn` 组装车间（消费 `ResetBattle`，不认识按键）

领域：`world` · `voxel_render` · `movement` · `combat`（8 子域）· `skills`（静态定义）·
`equipment`（装备）· `timeline` · `clock` · `ai` · `input` · `interaction` ·
`presentation` · `spawn`。

域地图与跨域契约见 [`docs/domain.md`](docs/domain.md)，文档入口是
[`docs/index.md`](docs/index.md)。

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

## 待办

### 工程债（按优先级）

- [x] **动作数值外置**（本次，第一版）：`config/actions.ron` 一份文件描述所有动作数值，
      由 `src/config/` 装载成 `ActionConfig` 资源。
      **分层决定（按你的指示）**：配置**独立一个目录、不放进 `assets/`**——
      `assets/` 是素材（换皮），`config/` 是数值（换手感）；发行时可以不带上它，
      热重载素材时也不会把数值一起卷进来。装载走**普通文件读写**（`PreStartup`），
      不是资产管线：数值要在**各域注册技能定义之前**就绪，走资产管线会引入
      "异步加载好了没"的时序问题，而这是本地小文件、同步读没有代价。
      **失败策略**：文件不存在 → 内置默认值（= 原有常量，所以没有它也能跑）；
      解析失败 → **大声报错**（带行号）并退回默认值，**不 panic**
      （打错的数字不该让游戏起不来，但也不该被静默忽略）。
      **一处真相**：各域常量**保留**为"默认值"，配置是**覆盖**；目录、声明系统、
      帮助面板全从 `ActionConfig` 读，改一处全都跟着变。
      **改到的链路**：8 条技能定义（移动 / 跳跃 / 翻滚 / 横扫 / 箭矢 / 火球 / 招架 / 等待）
      + 7 个声明系统（`declare_move` / `move_to` / `jump` / `roll` / `melee` / `fireball` / `parry`）
      + Focus 上限与回复间隔 + 三种移速。
      验收：`a_changed_config_reaches_the_catalogue_and_the_declaration`——
      三段分别钉**目录**、**行动实体**、**玩家按键声明**都拿到了配置里的值；
      **验过不是空跑**（让 `melee_timing` 忽略配置后，第三段立刻转红，
      报"实际 [0.35]"）。
      **仍未做**：热重载（改文件要重启）、`ActionRegistry` 让 HUD 从注册表读
      （现在仍是 `SKILLS` 数组）、伤害数值（`PhysicalDamage` 仍写在各载荷里）。
- [x] **死代码清理**：删除 `defense/actions.rs` 里重复且未注册的
      `expire_defense_markers_system`、`lifecycle::manage_projectile_hits_system`、
      写而无消费的 `AttackResolved`。剩下的 `declare_skill_system` / `arrow_scene`
      当时是「单体狙击」的预留实现（收尾已与火球对齐）——**现已接上输入**，
      见本页的「箭矢接回输入（弓：单体狙击）」（那个从未注册的
      `declare_skill_system` 已被真正的 `declare_shoot_system` 取代并删除）。
- [x] **`Space` 手动暂停只前进一帧**：改成暂停原因集合 + **每帧断言**，手动暂停一直有效
      直到再按一次；"按一下是暂停还是恢复"的闩住在
      `input::keyboard::pause_input_system`（读 `PauseReasons`）。
- [x] **箭矢的收尾方式**：`shoot_action_executor_system` 现在忙到
      `距离 / ARROW_SPEED`（与火球共用 `DecisionSlot::recovering(.., effect_delay)`）。
- [x] **玩家身份收口**：新增 `timeline::InputDriven` 标记（组装层挂在玩家身上），
      时间线 / 反应系统 / 撤销 / 各玩家声明系统都认它，不再满世界找 `Faction::Player`。
      表现层仍按 `Faction` 找单位——它看的是阵营，不是输入归属。
- [x] **反应窗口的粒度**（本次查明：**原条目的前提不成立**）：条目说"多段攻击
      （连续三刀）只会问玩家一次"，**实测不是这样**——威胁源是**实体**，
      每个来源各自开窗：三刀同时压过来时，玩家答一刀、那一刀落地，
      下一刀立刻开新窗重新冻住。既不会漏问，也不会一次问三遍。
      原来的"改成 `ReactionSlot` 存集合"**因此不必做**——集合已经隐含在
      "每个威胁一个实体"里了。
      **真正的多段攻击仍不存在**：那需要**一条**行动声明**三个**落地时刻
      （现在一条行动只有一个 `execute_at`），属于 `PendingHit` / `ActionTemplate` 的范畴。
      验收：`each_of_several_threats_gets_its_own_ask`（**验过不是空跑**：
      加一个"问过就不再开窗"的闩后转红）。
- [x] **威胁窗口存的是行动者而不是行动 → 死锁**（已修，M26）：`ThreatWindow.opening_action`
      存的是**玩家实体**，`detect_threat_system` 靠"玩家那一手变了没有"推断表态——
      玩家在**前摇中**被威胁冻结时，撤销再声明一手不会改变这个值，窗口于是永远等不到
      表态，**双方一起冻死**。M26 的 `ReactionSlot` 换成显式表态通道
      （`ReactionAnswer::{Counter, Abandon}`），这个判据整条删掉了——
      不再依赖任何"变了没有"的间接推断。
- [x] **AI 不会用 Focus**（本次）：**根因是 `Focus` 曾经是全局资源**——只有一份，
      敌人不可能有自己的。改成**每单位一份的组件**（`Focus` Component +
      `FocusRecoverTimer` 也随单位走），**玩家与敌人对称**。
      AI 的**闪避**（`Tactic::Dodge`）走共用入口 `ScheduledAction::with_focus(.., true)`：
      威胁已经在前摇了，等一个正常前摇再滚就来不及——买掉前摇才闪得开。
      顺带：`counter_suggestions` 现在读**被威胁玩家自己的** Focus（以前读全局的）。
      验收：245 测试全绿 / clippy 零警告 / fmt 通过；
      `a_dodging_enemy_spends_focus_to_zero_its_windup` **验过不是空跑**（不花就转红）。
- [x] **`AttackFrame` 没有消费者**（已接上）：预演读数现在带上**速度帧**
      （`FIREBALL · cell (1,0) · dist 2.2 · dmg 12 · frame 7`）——那就是
      「谁先动」的洞察力读数，字段因此有了真正的消费者。
      顺带把散落的帧数提成常量（`FIREBALL_FRAME = 7` / `ARROW_FRAME = 4`，
      与既有的 `MELEE_FRAME = 5` 同形），载荷与技能表共用一份。
      **技能表的 `frame` 与载荷上的 `AttackFrame` 是对账过的**——
      `the_frame_matches_the_frame_on_the_payload` 钉住两处不许分叉
      （验证过：故意改一处后确实转红）。
      验收：240 测试全绿 / clippy 零警告 / fmt 通过。
- [x] **没有生产者的预留类型**（本次）：`Voxel` / `VoxelPos` **早已不存在**（旧条目过时）；
      `ChunkPinned` 的缺口是真的，而且掩盖了一个**更严重的 bug**：
      ① `voxel_at_ground` 只在 `from_voxel(.., 0, ..)` 这一层区块里找地表，
      而默认地形在 y ≤ 0（区块 y=0 是空气、y=-1 才是土）——于是**每一格都被拒绝**
      （`BlockRefused::TerrainNotLoaded`），玩家按 `B`/`V` 在世界里**哪里都放不了/挖不动**。
      改成沿这一列**已加载**的区块从上往下扫。
      ② `set_voxel` 现在顺手挂 `ChunkPinned`（钉住 + 标脏绑在同一处，
      后来的调用方**漏不掉**）：否则走远再回来，改动会被噪声重新生成覆盖掉。
      ③ `BlockRefused` 原来**只有生产者没有消费者**（文件注释却写着"消费：提示条"）——
      接进 `presentation` 的 `ActionHint`，与 `ActionBlocked` 共用同一条提示条。
      验收：248 测试全绿 / clippy 零警告 / fmt 通过；
      三条新测试都**验过不是空跑**（分别还原旧实现后转红）。
- [x] **开发热重载**（本次）：**原来的"环境阻塞"是误判**——`notify-debouncer-full 0.7.0`
      本来就在 USTC 镜像里（`0.6.0` 是本地索引缓存的旧快照；不改 `Cargo.toml`
      就不会刷新）。现在 `Cargo.toml` 里是一个**具名 feature**：
      `cargo run --features hot-reload`（生产构建不带它，文件监视器不进依赖树）。
      验收：`cargo check --features hot-reload` 通过、`cargo tree --features hot-reload`
      含 `notify-debouncer-full v0.7.0`；不带 feature 时依赖树里没有它。
      **教训**：报"环境阻塞"之前先改一次 `Cargo.toml` 再解析——见 `AGENTS.md` 的环境注意事项。

### 玩法与表现

- [x] **箭矢接回输入（弓：单体狙击）**（本次）：`ShootAction` / `arrow_scene` 早就
      实现且被测试覆盖，但**整条提交路径是死的**——`declare_skill_system` 从未注册进
      插件，`shoot_action_executor_system` 也没注册，`menu.rs` 里 `SkillKind::Shoot`
      那一分支只写了句注释。现在补齐：`ShootCommand`（新消息）→
      `declare_shoot_system`（与火球同形：`can_cast` 校验、配置读节奏、武器偏移）→
      `shoot_action_executor_system`（已注册）→ 箭矢命中。
      **箭矢进技能栏第 5 格**（`1`~`5` 直接放），与火球的分工是数据说的：
      火球锁格 + 半径 AoE、箭矢**单体**、伤害 10（火球 12）、帧 4（火球 7，更快出手）。
      `declare_skill_system`（那个"从未注册的参考实现"）**删掉**——它是死代码，
      它的两个触发源现在各有真正的归宿。
      **顺带修掉两个真 bug**（都是接上输入之后才暴露的）：
      ① `SHOOT_ABILITY.power` 写着 `MELEE_DAMAGE`(15)，而箭矢实际打 10——
      `menu_matches_the_catalogue` 立刻抓到了这个分叉；
      ② **射弹碰撞用三维距离**：单位站在地表上、`y` 随地形起伏，而箭从射手脚底平飞，
      一格之高差就够让箭"擦着头皮飞过去"（实机：完全打不中）。改成**地面平面距离**
      （与 AoE 的判据同源：决策按格、结算按地面距离）。
      ③ 技能栏**手写了 4 个 `skill_slot`**，目录加到 5 条时静默少一格
      （按得出来、栏里看不见）。改成 `Children::spawn(SpawnWith(..))` 按目录长度迭代建，
      并把那条空跑的测试（只断言 `SKILLS.len() == 4`）换成**数真实 `SkillSlot` 实体**。
      验收：308 测试全绿 / clippy 零警告 / fmt 通过；
      `the_bow_slot_declares_a_shot_that_actually_hits`（看**实际掉血**）、
      `an_arrow_hits_only_the_enemy_it_was_aimed_at`（两个敌人都在火球半径内，
      只有一个掉血）、`the_bar_builds_one_slot_per_ability`；
      后两条**验过不是空跑**（把碰撞改回三维距离 / 把槽位 `take(4)` 后都转红）。
      **实机**：BRP 读技能栏 5 格、按 `5` 后敌人血量 50 → 15（两箭），
      截图确认第 5 格图标在栏里。
- [x] **可行走性判定**（本次）：移动此前**完全没有**可行走性检查——点哪走哪、
      一路直线穿过任何东西。现在声明时先问"那一步迈得上去吗"，
      迈不上去就写 `MoveRefused::BlockedByTerrain`（HUD 提示条显示 `BLOCKED`）。
      **判据是纯规则**（`movement::rules::can_step`，零 Bevy、可单测）：
      只吃两格的**地表高度**，差 ≤ 1 个体素 = 一级台阶（可走），
      差 ≥ 2 = 墙（拒绝）；**下坡永远允许**（否则堆墙会把自己关在里面）。
      点地板走多格时**逐格**检查直线路径——只查终点的话墙可以被绕过。
      ⚠️ **默认地形永远不拒绝**（相邻格最多差 1）：这条规则真正起作用的地方是
      **玩家把地形堆高之后**。
      验收：`can_step` 的边界用例 + `a_target_behind_a_wall_is_refused`
      （放大起伏造墙，确认声明被拒、槽仍空、无行动实体）+ `every_cell_on_the_line_must_be_walkable`；
      前两条**验过不是空跑**（拿掉检查后转红）。
- [x] **真正的体素碰撞**（本次，按你的"可以打破"）：**站立高度现在看真实体素**，
      所以玩家堆的方块真的能站上去、真的挡路。这是本次最有分量的一处——
      在此之前建造**纯粹是装饰**（实测：连放 3 块石头，`ground_position` 纹丝不动）。
      **打破了什么**：原来"地形高度是纯函数、不依赖区块加载"那条性质。
      新规则（`world::storage::ground::ground_height`）：
      **区块已加载 → 看真实体素；未加载 → 退回噪声地表**。
      第二条是**兜底而非补丁**——流式范围外的列本来就该按"未改动"处理
      （玩家只能改看得见的区块），所以退回噪声是正确行为。
      **一处真相**：贴地、位移吸附、可行走性全走同一个 `ground_height`，
      因此三者不会各算一套；改方块之后地面立刻变。
      验收（`world::storage::ground` 7 条）：放一块抬高、挖掉降低、未加载退回噪声、
      水不算实心、堆两层就**走不过去**（`a_wall_of_placed_blocks_is_not_walkable`）；
      **实机探针**：放 3 块方块后站立高度 `0 → 3`（改动前是"纹丝不动"）。
      **未做**：扫描半径 `GROUND_REACH = 8`（堆更高 / 挖更深会按噪声收尾）、
      寻路（仍是直线）、站立容差（贴到相邻格边缘不判定）。
- [x] **命中特效**（本次）：一次命中的地方冒一小簇**短命粒子**（上浮 + 缩小，
      `EFFECT_SECONDS` 后自己销毁），颜色取**被打中的那一方**的阵营色
      （打中敌人是红、打中玩家是蓝）——一眼看出打中了谁。
      **落在 `presentation`**（与 HUD / 日志同级）：**只读** `DamageEvent`，
      不改任何游戏状态、不影响结算，所以 `combat` 不需要知道"命中要好看"。
      **不引粒子库**：这一版只有一种特效，为此加 `bevy_hanabi` 会付一整个依赖树的代价
      （项目规矩：够用就不加机制）。用最朴素的做法——`spawn` 几个小方块、
      一个系统让它们动、到点自己 `despawn`（"自己收尾"与各执行器同一条纪律）。
      **触发条件**（满足任一条就该换成真正的粒子系统）：需要几十个以上粒子 /
      复杂发射形状 / 拖尾光照 / 多套特效资产。本模块对外只有"读 `DamageEvent`"一个接口，
      届时整体替换、外部不受影响。
      **用虚拟时间推进**（不是真实时间）：世界冻结时特效定格在"刚打中"的样子，
      玩家解冻后接着看完——用真实时间的话，等玩家思考时特效会自己播完，反而看不清。
      验收：330 测试全绿 / clippy 零警告 / fmt 通过；四条新用例
      （自己会消失 / 从**被打中那个单位**身上冒出 / 目标没了不 panic / 会上浮缩小）。
      **实机**：截到了命中处那簇粒子（红色，落在敌人脚下）。
      ⚠️ **排查时踩到自己的老坑**：第一次实机"看不到粒子"，是因为
      **`EffectParticle` 没派生 `Reflect`——BRP 查未注册的组件会静默返回空**，
      看着像"没生成"。派生之后就查到了（`world.query` 返回 5 个）。
      这类"查不到 ≠ 不存在"的坑记在 `docs/bevy-019.md` 与 BRP 那几条记忆里。
- [x] **护甲接进组装层**（已落地，M27 后为**基础值**）：公式（第 ④ 关）早就支持
      `Armor`，但**没有任何单位挂它**——减伤永远是 0，等于这一关不存在。组装层给玩家
      1 点、敌人 0 点（`PLAYER_ARMOR` / `ENEMY_ARMOR`，`armor_for(faction)` 是唯一出口）。
      M27 之后它是**基础值**：装备域在它之上加 `EquipmentBonus`（一身起始装备 +2），
      命中公式读两者之和。
      ⚠️ **数值是占位、该由你定**：参照当前数值（近战 15 / 火球 12 / 箭矢 10、50 血），
      玩家 1 点**不改变任何一击的刀数**（安全值），敌人 0 点保持"敌人更脆"的手感。
      验收：`the_assembled_player_actually_has_armor` /
      `the_equipment_armor_bonus_reaches_the_hit_formula`（后者**验过不是空跑**：
      还原成裸 `Armor` 后转红）。
- [x] **BRP 看不到战斗数据**（本次修好）：`Health` / `Stamina` / `Armor` /
      `DecisionSlot` / `Focus` 现在都能在运行时直接读。
      **纠一处我自己的误判**：我先前写"要加 `Reflect` 派生 + `register_type`"，
      实测下来**`register_type` 不是必需的**——本项目启用了 bevy 的
      `reflect_auto_register`，**派生 `#[reflect(Component)]` 即注册**；
      插件里那几行只起"自文档"作用（留着了）。真正卡住 BRP 的是**没派生**。
      **实测证据**（运行中的游戏，BRP 查询原始输出）：
      `Armor` 读到玩家 `1` / 敌人 `0`、`Health` `50/50`、`Stamina` `5/5`、
      `Focus` `3/3`、`DecisionSlot` `{"Idle":{"intent":null}}`。
      验收：`the_combat_state_is_visible_over_brp`（把 `Health` 的派生拿掉就转红）。
- [x] **`assets/textures/ground/grass.png`：审计结论 —— 这张图不该接**
      （本次，**主动不做**）。本条原本是"接入它"，动手前先看了它到底是什么：
      它是 **Kenney Prototype Textures** 的 `PNG/Green/texture_01.png`——
      素材名里的 "Prototype" 指的是**灰盒原型贴图**，这张图**正是一个灰盒格子**：
      纯绿底 + 白色网格线 + **烙在图上的白色文字「WALL / 1 × 1 meter / 1024 × 1024」**
      （实测：左上 200×130 区域里 9.5% 的像素是近白色）。
      把它铺到体素表面上，**那段说明文字会跟着重复铺满整个地形**。
      **为什么当初会入库**：加素材那次（d2e78f5）是按"Kenney 的 CC0 绿贴图"收的，
      没看内容——`assets/LICENSES.md` 里记的来源没错，是**选品错了**。
      **处置**：代码里**不加引用**（引用了才是把错误固化进渲染路径）；
      在 `assets/LICENSES.md` 与 `docs/assets.md` 写明"这是灰盒模板、不要接"。
      **要做真地表贴图时需要什么**：一张**可平铺**（tileable）的自然纹理，
      且要连同上面那条"纹理图集 / UV"一起做（网格现在没有 UV 属性，
      贪婪网格化之后还要按矩形尺寸铺开）。触发条件是那条 UV 待办被做掉。

### 渲染优化（`voxel_render`）

- [x] **贪婪网格化**（本次）：同类型共面的面合并成矩形。
      **先量后做**：动手前探针实测默认地形——区块 `y=-2`（整层石头）逐面输出
      是 `6144` 个四边形，合并之后只要 `6` 个，所以这一刀值得下。
      做法是**按面方向切层**（六个方向各扫 32 个切面），每层建一张「这一格要画
      什么类型」的掩码，先沿 `v` 拉一条、再沿 `u` 复制这条矩形——**只合并同类型**
      （不同材质要分到不同网格）。被挡住的格子不进掩码，所以**合并内含面剔除**。
      最容易错的一处是**绕序**：四个角是手拼的，`u × v` 必须等于法线，
      否则背面剔除会把整片面从里面画出来（而顶点数、面积全对）。
      逐面那条路**保留**（`cull_hidden_faces = false` 走它），既是"关掉优化看原始
      几何"的开关，也是贪婪实现的**对照**。
      验收：319 测试全绿 / clippy 零警告 / fmt 通过。四条新用例：
      `greedy_merging_covers_exactly_the_same_surface_as_per_face`（**对照验收**：
      两条路覆盖的**面积**相等，且合并确实减少了面数）、
      `a_solid_chunk_layer_collapses_into_single_rectangles`（实心区块 6 个面各并成
      **一块**，逐面会是 6144 个四边形）、`greedy_does_not_merge_across_different_types`、
      `every_greedy_quad_winds_counter_clockwise_from_outside`。
      **踩到并纠正的一处自误**：第一版对照测试比的是"顶点集合相等"——那会把
      **合并成功判成失败**（合并的本意就是去掉内部顶点）；改成比面积才是对的。
      绕序那条**验过不是空跑**（把 `+Y` 的 `u`/`v` 对调后转红，而面积那条照样绿
      ——说明它抓的是面积抓不到的错）。
      **实机**：截图确认地形渲染与改动前一致，无 panic / 资产错误。
- [x] **方块贴图：每类一张程序生成的图（不需要图集）**（本次）：
      **先纠正原条目的错误前提**——它写着"换纹理图集、网格化代码不动"，两处都不对：
      ① **不需要图集**：图集解决的是"一张网格里要多种贴图"，而本项目**按方块类型
      拆网格**（`apply_meshing_result_system` 每类各一个网格实体、各挂一个材质），
      所以每类方块**直接有自己的贴图**，一个材质一张图。
      ② **网格一直缺 `ATTRIBUTE_UV_0`**，所以"网格化不动"从来是假的。
      **做的事**：给 `MeshBuilder` 加 UV；**UV 按矩形尺寸铺开**（合并出来的面跨 N 格
      就重复 N 次，逐面路径是 `(1,1)`）——否则一张图会被拉满整片地面。
      贴图**程序生成**（16×16 逐像素噪声，围绕基色 ±8%，哈希掺了类型下标所以各类
      不同），与 `shadow.png` / 技能图标同一条路子：自有素材、许可干净。
      采样器显式设 **`Repeat`**——默认 `ClampToEdge` 会把超出 `0..1` 的 UV 拉成
      边缘那一条颜色、整片糊掉。
      验收：345 测试全绿 / clippy 零警告 / fmt 通过。四条新用例：
      `merged_faces_tile_their_texture_by_the_rectangle_size`（实心区块顶面的 UV
      跨度必须是 32 而不是 1）、`per_face_quads_use_one_tile_each`、
      `the_texture_tiles_instead_of_clamping`、`the_texture_has_grain_around_its_base_colour`。
      **实机**：截图确认地形与放置的方块都有可见颗粒、且按格平铺（没有拉伸糊掉）。
- [x] **AO（顶点环境光遮蔽）**（本次）：`lighting` 从"面朝向明暗"补上**逐顶点**遮挡
      ——凹角（墙根 / 台阶内角）自然变暗，立方体不再是一块平色。
      **为什么逐顶点**：逐面只能说"整面多亮"，而一个面的四个角受的遮挡可以完全不同
      （墙角那面：靠内角被夹住、外角敞开），明暗过渡正是 AO 的可见效果。
      **判定与换算分开**：`occlusion_level` 返回 **`u8` 等级**（0..3），
      亮度由 `shade_of_level` 换算——因为 **AO 必须进贪婪网格化的合并键**，
      而浮点相等在键里既脆又难读（整数等级精确、可比较）。
      代价是"起伏处少并几块"，平坦处照旧全并（四个角都是等级 3）——有测试守这条。
      下限 `MIN_AO_SHADE = 0.55` **不归零**：全黑会让凹角看起来像洞。
      验收：325 测试全绿 / clippy 零警告 / fmt 通过。四条新用例：
      `a_vertex_at_an_inside_corner_is_darker_than_an_open_vertex`（墙根那一格的地面，
      靠墙的角比敞开的角暗）、`coplanar_faces_with_equal_ambient_occlusion_still_merge`
      （AO 相同的照旧并——反向守"AO 进键没把合并能力废掉"）、
      `both_paths_compute_the_same_ambient_occlusion`（贪婪与逐面两条路算出的 AO 一致）、
      加上 `lighting` 里的等级判据用例。
      前两条**验过不是空跑**：把 AO 从顶点色里去掉 / 从合并键里去掉，分别转红。
      **踩到并纠正的两处自误**（都记进了测试的注释）：
      ① 第一版对照测试按**顶点位置**比亮度，报出"分叉 0.70 vs 0.462"——那其实是
      同一个角属于**两个不同朝向的面**（`face_shade` 1.0 vs 0.66），AO 一样；
      键改成 `(位置, 法线)` 并除掉 `face_shade` 才是纯 AO。
      ② 第一版凹角测试用"同一平面并排的两块"——那是**凸**的、全敞开，
      压根不该有 AO（测试正确地失败了）；改成"地板 + 侧上方一格"才真的形成墙根。
      **实机**：截图确认地形与放置的方块渲染无异常、无 panic / 资产错误。
      ⚠️ **实机读不到 AO 梯度**：默认机位下同面内的明暗差很细微，
      而"叠两格"又落在同一格里看不出来——这条的可见效果由单测守（它们在
      同一个面内比两个顶点），实机只验了"没画坏"。
- [x] **方块交互**（本次）：`B` 在**悬停格**放一块石头、`V` 挖掉一块
      （`BlockCommand { cell, place }` → `world` 的 `apply_block_command_system`
      → `set_voxel` → `ChunkDirtyEvent` → 渲染层重建网格）。
      **分层**：`set_voxel` 是纯函数 + 查询；本系统是应用层（消费消息、把格换算成
      体素坐标）；按键住在 `input`，`world` 不认识 `KeyCode`。
      **world 仍是纯数据域**：被拒的原因用**自己的** `BlockRefused`，不写时间线的
      `ActionBlocked`——否则裸 `MinimalPlugins` 单测会因缺消息而 panic（踩到了）。
      ⚠️ **键位（B/V）是暂定**：方块交互属建造能力，等做建造玩法时该外置成配置。
      验收：244 测试全绿 / clippy 零警告 / fmt 通过。
- [x] **区块持久化：只存「改动」，不存整块地形**（本次）：
      **为什么不是"保存区块"**——一个区块 `32³ = 32768` 个体素，而其中绝大部分是
      **噪声函数的确定输出**（地形是 `seed + 坐标` 的纯函数），存它等于把算得出来的
      东西抄一遍。所以存的是**改动**（`(世界体素坐标, 类型名)`），加载时先按噪声生成、
      再把改动**盖上去**。于是存档大小 = 玩家真的动过几格（几十条），与区块数量无关，
      而且"先生成后覆盖"的顺序天然正确。
      **格式 / 位置 / 时机**：一份 `.ron`（与 `config/actions.ron` 同格式同 serde），
      写在 `saves/world.ron`——⚠️ **与 `config/` 不是一回事**：那是设计数值（可手改、
      该进版本库），这是**玩家存档**（运行时产物、已加进 `.gitignore`）。
      **存类型名而不是下标**：下标会随"加一个新方块"整体挪位，那样旧存档会静默读成
      另一种方块；名字不认识时跳过那一条（报错比悄悄画错强）。
      **同格只留最后一条**（反复放挖不该堆成几十条）。
      **落盘时机**：退出时（`AppExit`），不是每改一格写一次盘。
      验收：339 测试全绿 / clippy 零警告 / fmt 通过。三条新用例守着**端到端**：
      `saved_edits_are_laid_over_the_generated_terrain`（先确认那格本来不是石头，
      再确认改动盖上去；还确认别处改动不污染这一块）、`an_edit_in_another_chunk_is_ignored`、
      `an_unknown_voxel_name_in_the_save_is_skipped`。
      **实机验了完整往返**：放方块 → 退出（存档写出 `edits: [1]`）→ 重开，**那块石头
      还在**；把存档挪走再开一次，同一位置就是干净地面（对照成立）。
      ⚠️ **踩到一个真 bug 并修掉**：`save_on_exit_system` 一开始挂在 `Update`，
      结果**退出时根本没存**（`saves/` 都没建）——`AppExit` 是**帧末**由 runner 检查的，
      而 `bevy_brp_extras` 的关闭消息也在那一帧才写出，`Update` 里的系统可能读不到它。
      改挂 `Last` 才对（实机复验：存档正常写出）。
      **触发条件**（满足任一条就该升级）：多存档槽 / 自动定时存 / 改动量大到 diff 列表
      不再便宜（那时改按区块 RLE 存 patches）。

### 字体与文本

- [x] **字体覆盖验收**（本次）：`tests/assets.rs` 现在逐字查 `cmap`
      （`the_font_covers_every_character_the_ui_can_show`），把"运行时人工看有没有豆腐块"
      变成自动验收。**原以为要新引第三方库，其实不用**——`skrifa` 本来就在
      `bevy_text` 的依赖树里，提成 `dev-dependencies` 不引入新的传递依赖。
- [x] **CJK 断行**（本次收口：**那条告警在这个依赖版本上根本不会出现**）：
      文档一直说运行时会刷 `ICU4X data error: No segmentation model for complex script`，
      **实测复现不出来**——把 `main.rs` 里对 `icu_segmenter` / `icu_provider` 的静音
      撤掉，打印中文上屏、跑完整局战斗，stderr 一行都没有。
      **根因**：那行错误住在 `icu_segmenter::complex::select` 的 `ChineseOrJapanese` 分支，
      而当 `ja` 模型缺失时才走到——可 `parley` 0.9 根本不用复杂脚本分段器：
      它调的是 `LineSegmenter::new_for_non_complex_scripts`（见
      `parley/src/analysis/mod.rs`），中文按 `cjdict` 的规则分段，**那条报错分支
      永远不执行**。所以：
      ① 静音已从 `main.rs` 撤掉（留着的唯一理由是软件渲染的 wgpu 告警）；
      ② 顺手验过"加 icu_segmenter + auto"这条路**能编译**（`cargo tree -e features`
      看到 `auto` 统一到 2.3.0），但**没有必要**——它解决的是一个不存在的问题。
      ③ 中文分词本身正常：「敌人向你发射火球」切成 `敌人 / 向 / 你 / 发射 / 火球`。
- [ ] **中文 HUD 文案**：字体已就位，把 HUD 文案翻成中文还需要中文排版
      （断行 / 标点挤压）。
- [x] **字体体积：已子集化 8.3 MB → 36 KB**（本次）：只保留**界面上真的会显示的字**
      （258 个字符：HUD 的 ASCII + 战斗日志那几个中文词 + 阵营标签），
      用 `fonttools` 的 `pyftsubset` 打出来。子集化**有测试兜底**——
      `tests/assets.rs` 的覆盖验收逐字查 `cmap`，而它的字符集**从源码抽**，
      所以"改了日志文案但忘了重新子集化"会当场转红，不会等到运行时变豆腐块。
      **为什么能这么小**：界面上真正显示的 CJK 只有战斗日志正文（HUD 是英文），
      一共不到 20 个汉字；之前的 8.3 MB 是"简体常用字全覆盖"。
      **可复现**：脚本写进了 `assets/LICENSES.md`，
      **实测过能逐字节重建出仓库里这一份**（`cmp` 通过）——没有能复现的配方，
      下一次改文案的人就只能瞎试。
      **丢掉了什么**（写在文档里）：hinting（屏幕字号下无所谓）、`DSIG` 签名、
      不用的 OpenType 布局特性。⚠️ **触发条件**：若将来要**用户可输入**的文本
      （聊天 / 命名），撤掉这个子集字体，换全量或按输入范围重做。
      验收：3 条资产测试全绿（含逐字覆盖）/ 325 单元测试全绿 / clippy 零警告 / fmt 通过；
      **实机**：截 HUD 正常（英文无豆腐块）、打一场确认中文日志正常渲染。

### 策略深度（目标形态，未落地）

- [x] **多敌人战斗：HUD 面板**（本次）：面板原来"后遍历到的敌人覆盖前面"，而
      **ECS 查询顺序不保证**——两个敌人时显示谁全凭运气，血条看起来自己在跳。
      现在定了一条由数据决定的规则：**显示离玩家最近的敌人**，并列时取格坐标小的
      （与遍历顺序无关）。验收：3 条测试，且**还原成旧行为后确实转红**。
- [x] **多敌人战斗：AI 侧（已具备）**：核实过 AI 本来就是**按实体**跑的
      （`Query<…, With<EnemyBrain>>` 逐个遍历、各自选最近目标、各自看威胁），
      探针实测两个敌人**各自独立移动**。所以这一项**只剩 HUD**，不需要改 AI。
- [x] **多敌人战斗：面板扩成 N 行**（本次）：**先让它能发生**——开局本来就只出一个敌人，
      所以"面板要画谁"这个问题根本不会出现。现在开局出**两个**（`spawn::enemy::ENEMY_SPAWNS`，
      各自一格、分开摆），面板才真的有多个可画。
      **模型从"两个固定槽位"改成"玩家一格 + 敌人 N 格"**：`PanelSlot::{Player, Enemy(index)}`，
      敌人按**离玩家最近**排序（规则与旧版完全一致，只是从"挑一个"变成"排全部"——
      所以只出一个敌人时显示谁没有变化），取前 `MAX_ENEMY_ROWS` 个；
      超出的按名次丢掉（不是随机丢）。
      **行池**：敌人那列按下标建好实体、每帧只改内容与显隐（与时间轴色块池同一个做法），
      所以敌人数量变化**不增删实体**。行从下往上排：第 0 行（最近的）永远在最下面，
      位置不会因为数量变化而整列乱跳。行名带名次（`ENEMY 1` / `ENEMY 2`）。
      **顺带发现并修掉字体验收的一处空转**：它从源码抽中文字面量时**连测试模块的
      断言消息也算进去**，于是"语序应当…"这类**永远不会渲染**的字被要求进字体
      （实测多要了 12 个）。改成**跳过 `#[cfg(test)]` 模块**（花括号配平），
      并验过它**仍然抓得住真的界面文案**（往日志的非测试代码里注入一个字体没有的字 → 转红）。
      子集字体重打：12 KB（131 个字形，含**全部可打印 ASCII**——HUD 是英文，
      任何字母都可能出现；这次就是 `dash` 的 `d` 不在里面被测试抓出来的）。
      验收：348 测试全绿 / clippy 零警告 / fmt 通过。四条新用例：
      `every_enemy_gets_a_row_ordered_by_distance_whatever_the_iteration_order`
      （两个敌人**都有行**、顺序由数据决定、与遍历顺序无关）、
      `enemies_beyond_the_row_pool_are_dropped_by_rank`（超出的按名次丢）、
      `a_missing_slot_reads_as_empty`（含越界名次不 panic）、
      `a_single_enemy_is_still_shown`（多敌人改动不影响只有一个）。
      **实机**：BRP 读到两行都 `display: Flex`、第三行 `None`；
      `ENEMY 1 · cell (3,3) · dist 7.2` 在 `ENEMY 2 · cell (3,5) · dist 10.8` **之上**（更近的在前面），
      截图确认右下角两行都在。
- [x] **资源分线：弹药线**（本次）：**精力与弹药分成两条恢复速度不同的池子**——
      这才是分线的意义（同一条池子里做不出两种手感）：
      | 池子 | 服务什么 | 恢复 |
      | :--- | :--- | :--- |
      | **精力** `Stamina` | 防御与机动（翻滚 / 招架 / 冲刺） | 每次重新可决策 **+1**（快） |
      | **弹药** `Ammo` | 远程与重击（火球 / 箭矢） | 每 **6 秒 +1**（慢，上限 3） |
      伤害手段因此是"**这一局还能开几炮**"的预算，防御手段是"**这一回合还能不能再滚一次**"
      的即时取舍。**平 A（近战横扫）不花任何资源**——这是分线的安全阀：
      "两条线都空了"永远还有事可做。
      架构上把"花费"收成一个枚举 `ResourceCost::{Free, Energy(n), Ammo(n)}`：
      它同时回答"花什么、花多少"，三处（定义 / 校验 / HUD）都读它，
      而**条件（`Requirement`）与花费是两件事**——走一格要求有精力却**不花**精力。
      **顺带纠正一处旧注释**：`Stamina` 的文件头一直写着"弹药留给火球 / 重击"，
      但代码里火球花的是精力——现在两者终于对上了。
      验收：353 测试全绿 / clippy 零警告 / fmt 通过。四条新用例：
      `affordability_filters_by_resource_line`（**分线的核心验收**：0 精力时近战仍可用、
      0 弹药时火球灭掉而翻滚照旧）、`ammo_recovers_much_slower_than_energy`、
      `ammo_recovers_only_while_the_world_runs`（冻结不回）、
      `the_state_line_carries_the_ammo`（面板读数）。
      **实机 A/B**：放一发火球 → 弹药 `3 → 1` 而精力**纹丝不动**（`5 → 5`）；
      面板读到 `PLAYER · ammo 1/3`、敌人 `ammo 3/3`。
      **未做**：架势槽 `Poise`（打断抗性 / 格挡那条线）——它是**资源线**而
      `CombatTags` 已经是标签闸门，两者重叠；等"格挡消耗架势"这类具体玩法
      落地时再定，避免先造一个没有消费者的池子（与 `PendingHit` 同一条判断）。
- [x] **格挡减伤**（M25 后半，已落地）：防御链第 ③ 关，与翻滚 / 招架并列。
      `BlockChance` 是单位属性的**基础值**（**来源是装备**，M27 已落地：
      一面盾给 +0.35，命中管线读"基础 + 装备加成"），
      纯逻辑 `resolve_block` / `blocked_damage` 零 Bevy 零随机。
      顺序按 `docs/combat.md`：① 闪避 ② 招架（拦下）→ ③ 格挡（按率**减伤**）
      → ④ 护甲再减；格挡照常触发打断（减伤不是免伤）。
      **没有 `Blocking` 标记组件**：格挡当帧结算完，状态落在 `DefenseOutcome::Blocked` 上
      （与跨帧的 `Dodging` / `Parrying` 不同；曾有过的空组件已删）。
      验收：`blocking_reduces_damage_instead_of_negating_it` /
      `partial_blocking_stacks_with_armor_in_the_documented_order`（挡 50% 得 3 而不是 4）。
- [x] **冲刺（位移 2 格）**（本次）：`X` + 方向键 → 朝该方向**一次冲两格**。
      **为什么是组合键**：冲刺**需要方向**（"朝哪冲"），而方向本来就由方向键表达；
      单键冲刺只能在"朝最近敌人 / 朝悬停格"里选，两种都会让"我想往那边冲"落空。
      `Shift` 已经让给 Focus 了，所以用了 `X`。输入域照旧只翻译：`movement` 收到的是
      `DashCommand`，**不认识 `KeyCode`**。
      **分工是数据说的**：走一格 0.15/0.10 前摇后摇但只走一格；冲刺 0.25/0.20 前摇更重
      却跨两格——**用时间换距离**，追人 / 脱离时用。速度 7.0（走路 5.0），
      于是"跨两格但用时更短"由**速度**说了算，不需要第二套位移逻辑。
      **它是一条普通技能**：注册进目录（`AbilityId::Dash`）、走 `can_cast` 校验、
      节奏与花费从 `config/actions.ron` 读（`dash` 段 + `speeds.dash`）、
      标签 `COMMITTED`（蹬出去收不回来，与跳跃 / 翻滚同族）。
      **逐格查可行走性**：跨两格时中间那格也得迈得上去——只查终点的话墙能被跨过去
      （与点地板走多格同一个坑）。
      **未做**：`范围攻击排程`（那半条要的是"一次行动打多个落地时刻"，属于 `PendingHit`
      的范畴，而 `PendingHit` 的审计结论是"现在不建"——两者触发条件相同）。
      验收：347 测试全绿 / clippy 零警告 / fmt 通过。
      `dashing_moves_two_cells_in_one_action`（`X`+方向 → 一条 `DashAction`、
      **不是** `MoveAction`、落到 +2 格）与 `a_dash_through_a_wall_is_refused`；
      两条都**验过不是空跑**（把 `* DASH_CELLS` 改成走一格 / 拿掉可行走性检查，各自转红）。
      **实机 A/B**：同一个方向 —— 普通方向键走 **1 格**、`X`+方向键走 **2 格**。
- [ ] **`ActionTemplate` 资产图**：动作的静态定义（相位 / 消耗 / 效果 / 可取消规则）
      资产化，按边条件在图上转移。当前已落地的最小形态是
      `ActionTiming { windup, recovery }` + 行动实体。
- [x] **延迟命中实体化（`PendingHit`）—— 审计结论：现在不建**（本次）：
      **没有任何消费者**。查过三处：① 场上不存在延迟 AOE / 地面效果 / DoT（`grep` 全库
      只有 TODO 自己提到它）；② 现有"效果晚点发生"的动作**已经解决得挺好**——
      `effect_delay` 让行动者忙到效果真正发生（火球忙到落地、箭矢忙到命中），
      没有留下需要清理的临时方案；③ 与它相关的两个功能（多段技能、`ActionTemplate`）
      **本身都还没做**，先造基础设施就是"为假想的未来设计"。
      项目自己的规矩是 **"够用就不加机制"**（`docs/skills.md` 第七节），
      `CLAUDE.md` 也写着「不要为假想的需求设计」——所以这条**主动不做**。
      **触发条件（满足任一条就说明该做了）**：
      ① 出现**一条行动打多个落地时刻**的技能（连击三下 / 蓄力两段）；
      ② 出现**地面效果**（地火 / 毒圈）——它的伤害发生在未来、且与施法者解耦；
      ③ 出现需要**排到未来事件堆**并可按时间查询的玩法（预告圈、倒数）。
      届时的最小形态：一个 `PendingHit { at, source, target/area, amount }` 实体，
      由时间线按 `at` 触发——**不需要新调度器**，执行器那套 `due(now)` 直接复用。

### 信息层（G 层：信息即力量）

- [x] **洞察力 / 时间轴悬停读数**（设计稿 ①②③ 落地）：滑到时间轴的色块上，
      状态行下方弹出一行 **`ENEMY · fireball → cell (3,1) · 0.4s · interruptible`**
      ——谁 · 什么 · 打哪 · 还剩多久 · 能不能被打断；
      同时**战场上圈出那一手的主人**（脚边一个琥珀色圆环，`TimelineFocusRing`），
      时间轴与战场就这样连起来了。
      **一行新增数据类型都没有**（设计稿的关键发现兑现了）：那五个数本来就挂在
      行动实体上，只是玩家看不见。做法是让**色块自己带着行动实体的 id**
      （`TimelineSlot::action`），悬停时反查——比"按车道找那个行动者的行动"稳：
      后者的判据会与分道逻辑分叉。
      **对抗标签的读法与打断闸门共用一条口径**（`!interruptible || super_armor`
      → `super armor`）：读数说能打断、实际断不掉比没有读数更糟，有测试钉住。
      三层照旧新增 `readout.rs`（`readout_line` 纯函数 + `TimelineHover` 事实 + 三个
      标记组件），取数写 UI 在 `system.rs`，条子与指示圈本身在 `scene.rs`。
      **两处与设计稿的偏差**（都写进了 `docs/insight.md`）：读数显示的是**前摇剩余秒数**
      （不是前摇/后摇两个数——总时长在色块宽度里已看得见）；指示圈圈的是**行动者**，
      不是"威胁格"，因为格子高亮要与 `HoveredCell` 抢通道，留到下一轮。
      验收：315 测试全绿 / clippy 零警告 / fmt 通过；
      `hovering_a_block_shows_what_that_action_is`（整条链路：色块 → 反查 → 读数文案 →
      `TimelineHover` 带 action + actor）、`the_readouts_countdown_follows_the_virtual_clock`、
      `the_focus_ring_circles_the_actor_of_the_hovered_action`；
      前两条**验过不是空跑**（判据从 `Hovered` 改成只认 `Pressed` 后转红）。
      **实机**：指示圈实体在运行中可查、默认 `Hidden`、平铺旋转正确，无运行时错误。
      ⚠️ **实机限制**：BRP 的合成光标驱动不了 `Interaction`（Bevy 每帧重算它），
      所以"鼠标真的悬停上去"这一步只能靠测试守，实机只验到接线与默认隐藏。
      **未做**：画威胁格（设计稿第四节，与格子高亮同一套合并逻辑）、
      敌人面板展开式洞察力面板。
- [x] **战斗日志：两方标签贴在一起（实机发现的显示 bug）**（本次）：
      日志那一行读出来是 **「敌人玩家 命中，受到 16 点伤害」**——受击方**直接拼在**
      出手方那句话前面（`format!("{who}{text}")`，而 `text` 又以出手方开头），
      于是同一句里两个阵营标签**贴在一起**、中间连空格都没有，语序也反了。
      **是接字体子集那条活、跑实机打一场时看见的**——四条单测全绿，
      因为它们只断言"两个标签都出现"，不检查语序与分隔。
      现在语序是「出手方 → 受击方 → 数值」：`敌人 命中 玩家，造成 12 点伤害`。
      **测试改成钉整句**，并补一条 `the_two_side_labels_are_never_glued_together`
      （两个标签相邻即失败）——两条都**验过不是空跑**（还原旧写法后同时转红）。
      **连带**：新文案引入的"造成"两个字形不在子集字体里，
      **字体覆盖测试当场转红**（这正是它的用途），重新子集化即可（8.3 MB → 36 KB）。
- [x] **战斗日志：谁打的谁**（本次）：日志原本只写"玩家 受到 12 点伤害"，
      **看不出谁出的手**；死亡也只写"敌人 阵亡"，而 `DeathEvent.killer` 一直被忽略。
      现在写得出「玩家命中，受到 12 点伤害」（来源是攻击实体，读它的 `Faction` 认出手方）
      与「敌人 被玩家击杀」。环境伤害（`source: None`）照旧只写受击方。
      **历史 N 条本来就有**：`BattleLog` 环形保留 16 条、HUD 显示最近 8 条，
      "回看"这一半此前就已落地（旧条目把它和死亡复盘写在一起，容易误读）。
      **仍未做**：时间戳——"**哪个时刻**命中了谁"需要虚拟时间进 `DamageEvent`，
      它现在不带时间。验收：新增 5 条日志用例（**验过不是空跑**：把出手方抹掉后
      `a_hit_names_the_side_that_struck` / `a_death_names_the_killer` 转红）。
      **顺带修掉字体验收的空转**：`tests/assets.rs` 手抄了一份中文字表，
      这次新增的"命中/被/击杀"有 8 个字不在表里——已改成从**源码**里抽中文字面量，
      并验证过它真的会红（注入一个字体没有的 U+9FF0 后转红）。
- [x] **洞察力面板（设计稿 ④，取"乙"）**（本次）：敌人面板**每一行**多一条读数
      **`range 1 · break 1 · approach`**——射程 / 打断抗性 / 战术各回答一个具体问题
      （"我站哪儿安全" / "该躲还是该抢一手" / "它想干什么"）。
      **`break` 只在敌人真的有前摇中的那一手时出现**（读数来自**正在前摇的那条
      行动实体**），没有那一手就整段不出现——不留空段。
      **两处与设计稿的偏差**（都写进了 `docs/insight.md`）：
      ① **没做展开 / 收起**：设计把那当作"乙"的代价，实际直接列出来更简单也更有用，
      行数由 `MAX_ENEMY_ROWS` 兜着；② 玩家那一格**不显示**（读自己不需要它）。
      **没有新增数据类型**：`Insight` 只是把已有的三个数装在一起。
      验收：355 测试全绿 / clippy 零警告 / fmt 通过；
      `the_insight_line_carries_range_break_resistance_and_tactic` 与
      `missing_insight_readings_are_left_out_entirely`（缺的读数不留空段）。
      **实机**：面板读到两行都是 `range 1 · break 1 · approach`（敌人正在前摇时
      `break` 才出现，实测确认），截图确认两行都在、不重叠。
      **顺带修掉一个布局 bug**：敌人行原本是**逐行绝对定位 + 写死行高 76px**，
      而加了第五行内容之后自动高度被压到 **52px**、行会挤在一起——
      改成 **flex 列**（`column_reverse` 让第 0 行贴底）之后"几行、多高"由内容决定，
      再加一个 `min_height` 兜住文字量不准的问题。
      **这为"成长以知识为主"铺好了路**：信息已经看得见，剩下的只是权限机制。

## 依赖与文档索引

> 本机 crates.io 直连不可用，版本经**中科大（USTC）镜像稀疏索引**查询确认
> （配置在 `~/.cargo/config.toml`）。
> 依赖一律手动写入 `Cargo.toml`（`cargo add` 在镜像下不可用）；
> 新依赖确认版本后先登记下表再引入。
> ⚠️ 本地索引缓存**只在改过 `Cargo.toml` 后**才刷新：解析失败先改一次再试，
> 别急着判定"镜像没有这个版本"。

| 依赖 | 版本 | 用途 |
| :--- | :--- | :--- |
| bevy | 0.19.1 | 引擎（`Cargo.toml` 写 `0.19`，`Cargo.lock` 锁 0.19.1） |
| rand | 0.10.2 | 装饰物随机摆放 |
| bevy_brp_extras | 0.22 | 运行时调试协议扩展：截图 / 输入模拟 / 干净退出 |
| notify-debouncer-full | 0.7.0 | `hot-reload` feature 的传递依赖（`bevy/file_watcher`） |
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
