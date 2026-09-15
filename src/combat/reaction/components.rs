//! 反应系统组件：威胁声明（行动侧）+ 反应窗口状态。

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

/// 反应窗口：**一次威胁只开一个窗口**。
///
/// ```text
/// 威胁出现（无 → 有）  → 冻结，记下玩家当时那一手
/// 玩家换了一手        → 解冻（他表态了，那一击该来就来）
/// 威胁消失（有 → 无）  → 复位，等下一次威胁再开窗口
/// ```
///
/// 记的是"窗口打开时玩家那一手是什么"：只要玩家换了行动（撤销后重新声明、
/// 或旧的被打断 / 落地），就说明他表态了。用行动实体而不是时间戳，
/// 是因为**冻结时虚拟时间不动**，时间戳分不出先后。
///
/// `answered` 一旦置位就**不再重复开窗**（哪怕威胁还在）：否则玩家每换一手
/// 解冻一帧、下一帧又被冻回去，世界变成走一步停一步的抽风状态。
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ThreatWindow {
    /// 上一帧有没有威胁（用来识别"新出现的威胁"）
    pub threatening: bool,
    /// 窗口打开时玩家那条还没落地的行动（`None` = 当时玩家空闲）
    pub opening_action: Option<Entity>,
    /// 玩家是否已经就这次威胁表过态
    pub answered: bool,
}
