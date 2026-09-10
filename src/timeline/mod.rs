//! # timeline — We-Go 时间线（同步回合调度）
//!
//! 所有单位**同时规划、一起结算**：规划阶段虚拟时间冻结等玩家输入，玩家提交后
//! 时间推进一个固定窗口，行动实体按各自的 `execute_at` 到点执行，窗口结束回到
//! 规划阶段。
//!
//! ```text
//!              Enter（玩家提交）
//! Planning ─────────────────▶ Resolving ──（窗口结束，广播 RoundEnded）──▶ Planning
//! 虚拟时间：暂停               流动                                      暂停
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
//! 操作：`WASD` 声明移动、`Space` 声明射击、`E` 声明近战、`Enter` 提交本轮、
//! `R` 重置战斗。同一轮里后声明覆盖先声明（一个单位同一时刻至多一个行动）。

use bevy::prelude::*;

pub mod components;
pub mod events;
pub mod plugin;
pub mod resources;
pub mod systems;

pub use components::{Committed, Declared, Pending, ScheduledAction};
pub use events::{ActionsCommitted, RoundEnded};
pub use plugin::TimelinePlugin;
pub use resources::{Phase, RESOLUTION_WINDOW, Timeline};
pub use systems::{
    clear_declared_actions, commit_actions_system, end_round_system, pause_during_planning_system,
    scheduler_system,
};

/// 时间线在 `Update` 中的系统集（排在输入之后、AI 与执行器之前）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimelineSet;
