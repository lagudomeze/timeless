# backlog · combat（威胁 / 反应槽 / 战斗读数）

> 活的总览：[`../../TODO.md`](../../TODO.md) · 历史证据：[`../../CHANGELOG.md`](../../CHANGELOG.md)

- [x] **#53 威胁窗口读不出来：不区分「能打断 / 只能躲」**（读数已修）
      **现象**（实测序列）：敌人火球前摇中 → `FROZEN · threat`（技能栏翻滚变青）→
      右键解冻 → 火球起飞 → **又** `FROZEN · threat`，**UI 与第一次一模一样** →
      右键 → 落地，玩家 -11。顶上一行小字是唯一的提示。
      **设计判断（保持）**：**问两次是对的**——① 时还能打断 / 反制，③ 时只能躲或忍
      （`detect_threat_system` 把投射物也算作威胁，这是设计）。问题是 HUD
      **没把这两种决策区分开**，玩家读不出"这次该按什么"。
      **拍板（2026-09-24）**：这条读数**走中文**——判据是
      「**战斗事实用中文**（日志 + 窗口读数），**界面控件文案用英文**（面板标题 / 按钮 / 状态标签）」。
      **改法（已落地）**：
      ① `ReactionSlot` 加 `ThreatKind::{Incoming, InFlight}`，**开窗时**由
      `detect_threat_system` 定下来（行动实体带 `Threatens` → `Incoming`；
      投射物带 `TargetCell` → `InFlight`）——读数层**不重新判断**，那会与检测分叉；
      ② `presentation/hud/hint.rs` 加纯函数 `threat_hint(...)`，两档中文文案：
      `敌 fireball 锁定你（0.4s 后落地） · 可打断（E 翻滚躲 / 右键忍）` /
      `敌 fireball 已出手 → 落点 (3,1)（E 翻滚躲 / 右键忍）`；
      ③ 位置在技能栏上方（复用 `hud::hint` 的区域），**不参与 2 秒淡出**——
      它是持续状态，靠窗口活着（关窗即隐藏）；
      ④ **优先级**：被拒的输入 > 威胁窗口 > 预演读数，判据只写在
      `gather_hint_system` 一处。
      **顺带把 `hint.rs` 拆成两层**：`gather_hint_system`（取数 → `HintState` 资源）+
      `apply_hint_system`（写节点 + 计时）——它此前同时回答"取数 / 计时 / 配色 / 写节点"。
      **落点可视化由 #57 承担**（已落地）。
      **验收**：`the_two_threat_readouts_say_different_things`（两档文案必须不同、
      前摇给倒计时、出手给落点）、`a_threat_readout_does_not_fade_away`（跑 3 秒仍在，
      表态后立刻收起）、`a_blocked_input_outranks_the_threat_readout`、
      `the_threat_countdown_follows_the_virtual_clock`、`a_friendly_threat_is_not_called_an_enemy`。
      **未做**：给反制建议再加角标 / 放大（"见招拆招那一刻唯一入口值得抢眼"）——
      可读性目前来自技能栏已有的反制高亮，先把文案这一半做好。
      **未实机复跑**：需要敌人真的打出火球并让玩家挨一下。

- [x] **#49 投射物伤害丢出手方**（已修，证据见 CHANGELOG）
      **修法留档**：根因是"写的时候实体还活着、读的时候已经死了"——`explosion.rs` 写了
      `source: Some(blast.projectile)`，同一帧又把投射物 `despawn()`，而 `presentation/log.rs`
      靠反查那个实体的 `Faction` 认出手方 → 实体没了 → 走"无出手方"分支。
      现在来源升级成 `DamageSource { entity, faction }`（**阵营在产生伤害的那一刻就记下来**），
      `apply_damage_system` 把它透传给 `DeathEvent.killer`，日志**不再需要任何查询**。
      **教训值得记**：这同时解掉了"日志必须排在 combat 之后、且来源实体还得活着"这条
      **隐式时序耦合**——凡是"消费者去查一个可能已经不在场的实体"，迟早会踩这个坑。
