//! # timeline — 无回合时间线（能决策就决策）
//!
//! 没有阶段、没有轮次、没有状态标记：**每个单位只要决策槽是空的就能决策**，
//! 节奏由每个动作自带的前摇 + 后摇决定（[`timing`]）。
//!
//! ```text
//! [Empty] ──声明──▶ [Windup + 行动实体] ──到点──▶ 执行器落地 ──▶ [Recovery { until }]
//!    ▲                                                                │
//!    └──────────────── recovery_system（now >= until）────────────────┘
//! ```
//!
//! 行动者的三个阶段**直接写在决策槽里**（[`DecisionSlot`]），不由「有没有行动实体」
//! 或「有没有时间戳」推导：
//!
//! | 状态 | 含义 | 行动实体 | 可撤销 | 可打断 |
//! | :--- | :--- | :--- | :--- | :--- |
//! | [`DecisionSlot::Empty`] | 空闲，可以声明 | 无 | — | — |
//! | [`DecisionSlot::Windup`] | 前摇中 | 有 | ✓ | ✓ |
//! | [`DecisionSlot::Recovery`] | 后摇中 | 无 | ✗ | ✗ |
//!
//! 本域只做调度，**不感知载荷**：行动 = 独立实体，实体上只有调度数据
//! [`ScheduledAction`] + 载荷 + [`Cancellable`]。执行器住在各自的领域里
//! （[`crate::movement`] 与 [`crate::combat`]），新增动作时调度器一行不改；
//! 收尾也**不集中**——执行器自己销毁行动实体、自己把行动者推进后摇。
//!
//! ## 冻结：原因集合，而不是一个布尔
//!
//! ```text
//! frozen ⟺ PauseReasons 非空
//! ```
//!
//! 原因由各领域用 [`PauseRequest`] 加减：`"manual"`（空格）、`"slot_empty"`
//! （玩家空着决策槽）、`"threat"`（combat 检测到威胁瞄准玩家）。多个原因可以叠加、
//! 互不覆盖——手动暂停因此不会只前进一帧。**唯一**写 `Time<Virtual>` 的地方是
//! 帧末 [`ClockSet`] 里的 [`apply_clock`]。
//!
//! Bevy 每帧把虚拟时间拷进通用 `Time`，因此位移、投射物、`Lifetime`、后摇计时
//! 全部自动停表，各领域不需要任何 `if paused` 分支。
//!
//! 决策按**格子**、命中按**真实距离**，格边长见 [`timing::CELL_SIZE`]。

use bevy::prelude::*;

pub mod components;
pub mod decision;
pub mod events;
pub mod plugin;
pub mod resources;
pub mod schedule;
pub mod systems;
pub mod timing;

pub use components::{Cancellable, InputDriven};
pub use decision::DecisionSlot;
pub use events::{
    ActionBlocked, ActionCancelled, BlockReason, InterruptEvent, PauseRequest, TogglePause,
    UndoCommand, UseFocus,
};
pub use plugin::TimelinePlugin;
pub use resources::{
    FOCUS_MAX, FOCUS_RECOVER_INTERVAL, Focus, FocusIntent, MANUAL, ManualPause, PauseReasons,
    SLOT_EMPTY, THREAT,
};
pub use schedule::ScheduledAction;
pub use systems::{
    apply_clock, compute_manual_pause, compute_player_awaiting_system, interrupt_observer,
    interrupt_system, process_pause_requests, recover_focus_system, recovery_system, roll_3d5,
    track_focus_intent_system, undo_system,
};
pub use timing::{ActionTiming, CELL_SIZE};

/// 时间线在 `Update` 中的系统集（排在输入之后、AI 与执行器之前）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimelineSet;

/// 钟表系统集：每帧**最后**一段，唯一的 `Time<Virtual>` 写入点。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClockSet;
