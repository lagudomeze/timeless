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
//! 操作：`WASD` 声明移动、`Q` 火球、`E` 近战、`Space` 跳跃、`F` 翻滚、`V` 招架、
//! `1`~`4` / `Tab` 选技能、`G` 释放选中技能、`R` 重置战斗。
//! 默认**按下即生效**（`TimelineConfig::require_commit = false`）；
//! 需要「先声明再确认」时按 `F1` 打开开关，`Enter` 才提交。
//!
//! 决策按**格子**、命中按**真实距离**，格边长见 [`timing::CELL_SIZE`]。

use bevy::prelude::*;

pub mod components;
pub mod events;
pub mod plugin;
pub mod resources;
pub mod systems;
pub mod timing;

pub use components::{BusyRecovery, Committed, Declared, Pending, Ready, ScheduledAction};
pub use events::ActionsCommitted;
pub use plugin::TimelinePlugin;
pub use resources::{Timeline, TimelineConfig};
pub use systems::{
    begin_action, commit_bridge_system, end_action, recovery_system, scheduler_system,
    timeline_gate_system,
};
pub use timing::{ActionTiming, CELL_SIZE};

/// 时间线在 `Update` 中的系统集（排在输入之后、AI 与执行器之前）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimelineSet;
