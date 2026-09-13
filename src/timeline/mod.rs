//! # timeline — 无回合时间线（能决策就决策）
//!
//! 没有阶段、没有轮次：**每个单位只要能决策就决策**，节奏由每个动作自带的
//! 前摇 + 后摇决定（[`timing`]）。唯一会停下世界的情况是「玩家已就绪、
//! 正等他按键」——那时冻结虚拟时间（[`timeline_gate_system`]）。
//!
//! ```text
//! [Ready] ──声明──▶ [Pending] ──到点──▶ [Committed] ──执行──▶ [后摇] ──▶ [Ready]
//!    ▲                                                              │
//!    └──────────────────── recovery_system ─────────────────────────┘
//! ```
//!
//! 本域只做调度，**不感知载荷**：行动 = 独立实体，实体上只有调度数据
//! [`ScheduledAction`] 与状态标记（[`Declared`] / [`Pending`] / [`Committed`]）。
//! 载荷、工厂与执行器都住在各自的领域里（[`crate::movement::actions`] 与
//! [`crate::combat::skills::actions`]），新增动作时调度器一行不改。
//!
//! 「暂停等输入」只用 `Time<Virtual>` 实现：Bevy 每帧把虚拟时间拷进通用 `Time`，
//! 因此移动、计时器、生命周期全部自动停表，不需要任何手写阶段门控
//! （AGENTS.md：暂停用 `Time<Virtual>`，不要手写阶段门控）。
//!
//! 操作：方向键走一格、左键点地板 / 敌人、`1`~`4` 直接放技能、`Q/W/E/R` 热键、
//! `Space` 暂停、`F2` 循环反应窗口、`F5` 重置战斗。
//! **没有"确认"这一步**：声明即生效（[`commit_bridge_system`] 当帧把它升为
//! `Pending`）；反悔靠**打断 / 撤销**（[`interrupt_system`] / [`undo_system`]），
//! 只要还没到结算帧就能撤。
//!
//! 决策按**格子**、命中按**真实距离**，格边长见 [`timing::CELL_SIZE`]。

use bevy::prelude::*;

pub mod components;
pub mod events;
pub mod plugin;
pub mod resources;
pub mod systems;
pub mod timing;

pub use components::{
    ActionCost, BusyRecovery, CancelCost, Committed, Declared, Pending, Ready, ScheduledAction,
    Uncancellable,
};
pub use events::{
    ActionBlocked, ActionCancelled, BlockReason, CycleReactionWindow, TogglePause, UndoCommand,
};
pub use plugin::TimelinePlugin;
pub use resources::{ReactionWindow, Timeline, TimelineConfig};
pub use systems::{
    begin_action, commit_bridge_system, cycle_reaction_window_system, end_action, end_action_until,
    insert_on_actor, pause_toggle_system, recovery_system, scheduler_system, timeline_gate_system,
    undo_system,
};
pub use timing::{ActionTiming, CELL_SIZE};

/// 时间线在 `Update` 中的系统集（排在输入之后、AI 与执行器之前）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimelineSet;
