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
//! [`ScheduledAction`] + 载荷 + 可选的 [`Uncancellable`]，并以 `ChildOf` 挂在
//! 行动者之下。执行器住在各自的领域里（[`crate::movement`] 与 [`crate::combat`]），
//! 新增动作时调度器一行不改；收尾也**不集中**——执行器自己销毁行动实体、
//! 自己把行动者推进后摇。
//!
//! ## 冻结：原因集合，而不是一个布尔
//!
//! ```text
//! frozen ⟺ 本帧的 PauseReasons 非空
//! ```
//!
//! 原因是**断言式**的：谁这一帧还想让世界停着就写一条 [`PauseRequest::Pause`]，
//! 下一帧不再断言，原因自然消失——不需要谁去"撤销"。集合每帧重建，因此既不会
//! 留下没人摘的幽灵原因，多个原因（`"manual"` 手动暂停 / `"slot_empty"` 等玩家决策 /
//! `"threat"` 威胁逼近）又能叠加、互不覆盖。
//!
//! 按哪个键暂停、按一下是暂停还是恢复，是 [`crate::input`] 的事：它每帧断言
//! （或停止断言）`"manual"`，并可以在需要时写一条 [`PauseRequest::Resume`]
//! 清空此刻已收集的原因。**唯一**写 `Time<Virtual>` 的地方是帧末 [`ClockSet`]
//! 里的 [`apply_clock`]。
//!
//! Bevy 每帧把虚拟时间拷进通用 `Time`，因此位移、投射物、`Lifetime`、后摇计时
//! 全部自动停表，各领域不需要任何 `if paused` 分支。
//!
//! 决策按**格子**、命中按**真实距离**，格边长见
//! [`crate::movement::CELL_SIZE`]（格尺度是移动领域的真相，本域只是读它）。

use bevy::prelude::*;

pub mod components;
pub mod decision;
pub mod events;
pub mod plugin;
pub mod resources;
pub mod schedule;
pub mod systems;
pub mod timing;

pub use components::{InputDriven, Uncancellable};
pub use decision::DecisionSlot;
pub use events::{
    ActionBlocked, ActionCancelled, BlockReason, DecisionReady, InterruptEvent, PauseRequest,
    PlayerIntent, UndoCommand, UseFocus,
};
pub use plugin::TimelinePlugin;
pub use resources::{
    FOCUS_MAX, FOCUS_RECOVER_INTERVAL, Focus, FocusIntent, MANUAL, PauseReasons, SLOT_EMPTY, THREAT,
};
pub use schedule::ScheduledAction;
pub use systems::{
    apply_clock, compute_player_awaiting_system, interrupt_observer, interrupt_system,
    process_pause_requests, recover_focus_system, recovery_system, track_focus_intent_system,
    undo_system,
};
pub use timing::ActionTiming;

/// 时间线在 `Update` 中的系统集（排在输入之后、AI 与执行器之前）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimelineSet;

/// 钟表系统集：每帧**最后**一段，唯一的 `Time<Virtual>` 写入点。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClockSet;
