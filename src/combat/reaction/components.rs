//! 反应系统组件：威胁声明（行动侧）+ **已经惊动过玩家** 的记账。

use bevy::prelude::*;

use crate::movement::Cell;

/// 「这条行动威胁到哪些格」——**由行动自己声明**。
///
/// - 火球：飞过的格 + 落点（[`trajectory_cells`](super::trajectory_cells)）；
/// - 近战：正前方一格 + 左右各一格（[`melee_arc_cells`](super::melee_arc_cells)）；
/// - 移动 / 跳跃 / 撤退：不挂这个组件（走过去不构成威胁）。
///
/// 用「格」而不是「实体」：威胁问的是"我站的地方安不安全"，
/// 而决策层本来就是按格算的（见 docs/timeline.md 第二节）。
#[derive(Component, Debug, Default, Clone, PartialEq, Eq)]
pub struct Threatens {
    pub cells: Vec<Cell>,
}

/// 飞行中的投射物瞄准的格。
///
/// 投射物不是行动实体（它的"意愿"早就确定了），因此单独用这个组件声明威胁：
/// 一枚正在飞向玩家脚下格的火球，和一条还在前摇的火球行动一样危险。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TargetCell(pub Cell);

/// **这条威胁已经惊动过玩家了**——边沿触发的一次性记账。
///
/// 挂在**威胁源**（那条行动实体 / 那颗投射物）上。检测系统只对「还没有这个标记、
/// 且瞄着玩家」的来源开一次冻结；打上标记之后它就不再触发，于是：
///
/// - 玩家按空格（`PauseRequest::Toggle`）**真的能走**——那一击照常落地，代价自负；
/// - 同一个来源不会"你按一次它冻一次"，不需要玩家在窗口之外再表态。
///
/// **为什么记在威胁源上而不是全局**：全局记账（旧的 `ThreatWindow`）只能表达
/// "现在有没有威胁"，于是每帧都得重新判断"玩家表态了没有"——而那个判断依赖
/// "玩家那一手变了没有"，在后摇 / 不可撤行动期间永远为假。记在源上之后，
/// 「惊动过」是源自己的属性，与玩家在哪个阶段无关。
///
/// 挂标记的时机**必须晚于** `detect_threat_system` 的读取（同帧写入会漏检），
/// 因此它由 `[`mark_threatened_system`](super::mark_threatened_system)` 在检测
/// **之后**按本帧的检测结果打上；来源被销毁时标记随实体一起消失。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Threatened;

/// **反应窗口的状态机**——只有 `detect_threat_system` 读写它。
///
/// 它存在的理由是**别去读别人的资源来判断窗口开不开**：读"世界现在冻着吗"会让
/// 判据随帧序漂移（原因集合每帧重建，而同帧里谁先谁后是脆的）。窗口自己记状态，
/// 只把"玩家有没有放开世界"这一个外部信号（[`PauseReasons`](crate::clock::PauseReasons)
/// 里还有没有 `THREAT`）当作关窗的依据。
///
/// ```text
/// 没威胁          → 什么都不做
/// 威胁出现        → open = true，断言 Pause(THREAT)
/// 窗口开着、来源还在 → 继续断言（世界冻结时来源与移动都停在半路，窗口该一直开着）
/// 玩家按空格放开   → 集合里没有 THREAT 了 → open = false, dismissed = true
/// 来源消失（打断 / 落地）→ open = false, dismissed = false（下次威胁重新开窗）
/// ```
///
/// `dismissed` 是「**这次别重开**」：玩家已经用空格表过态（选择忍受），
/// 同一个来源不该立刻把世界冻回来——否则空格按了等于没按。
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ThreatWindow {
    /// 窗口开着吗（开着 = 本域每帧在断言 `Pause(THREAT)`）
    pub open: bool,
    /// 玩家已经放开过这个世界，同一次威胁不再重开
    pub dismissed: bool,
}
