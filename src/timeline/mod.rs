//! # timeline — 无回合时间线（能决策就决策）
//!
//! 没有阶段、没有轮次、没有状态标记：**每个单位只要决策槽是空的就能决策**，
//! 节奏由每个动作自带的前摇 + 后摇决定（[`timing`]）。
//!
//! ```text
//! [Empty] ──声明──▶ [Filled + 行动实体] ──到点──▶ 执行器落地 ──▶ [Filled + Busy]
//!    ▲                                                              │
//!    └──────────────────── recovery_system ─────────────────────────┘
//! ```
//!
//! 三件事都由**时间戳**推导，不再有 `Declared` / `Pending` / `Committed` 那样的
//! 三态标记（标记与时间戳打架是这一类系统的经典 bug）：
//!
//! | 状态 | 判据 |
//! | :--- | :--- |
//! | 前摇（可撤销 / 可打断） | `now < ScheduledAction.execute_at` |
//! | 该执行了 | `now >= ScheduledAction.execute_at` |
//! | 后摇 | 行动者身上有 [`Busy`] |
//!
//! 本域只做调度，**不感知载荷**：行动 = 独立实体，实体上只有调度数据
//! [`ScheduledAction`] + 载荷 + [`Cancellable`]。执行器住在各自的领域里
//! （[`crate::movement::actions`] 与 [`crate::combat::skills::actions`]），
//! 新增动作时调度器一行不改；收尾也**不集中**——执行器自己销毁行动实体、
//! 自己挂 [`Busy`]。
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
pub mod events;
pub mod plugin;
pub mod resources;
pub mod systems;
pub mod timing;

pub use components::{Busy, Cancellable, DecisionSlot, InputDriven, ScheduledAction};
pub use events::{
    ActionBlocked, ActionCancelled, BlockReason, InterruptEvent, PauseRequest, TogglePause,
    UndoCommand, UseFocus,
};
pub use plugin::TimelinePlugin;
pub use resources::{
    FOCUS_MAX, FOCUS_RECOVER_INTERVAL, Focus, FocusIntent, MANUAL, ManualPause, PauseReasons,
    SLOT_EMPTY, THREAT,
};
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
