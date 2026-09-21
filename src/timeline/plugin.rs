//! 时间线插件：注册资源、消息与观察者，并声明两段系统链。
//!
//! ```text
//! TimelineSet（帧中）：断言暂停原因（PC 没决定）→ 记 Focus 请求
//!                     → 撤销（右键与「玩家动手了」在此汇合）→ 后摇恢复 → Focus 回复
//! ClockSet  （帧末）：暂停请求 → 原因集合（每帧重建）→ apply_clock（唯一的时钟写入）
//! ```
//!
//! 本域**不认识按键**：手动暂停的"按哪个键、按一下是暂停还是恢复"由
//! [`crate::input`] 决定，它每帧断言（或不再断言）一条原因，这里只负责收。

use bevy::prelude::*;

use super::ClockSet;
use super::TimelineSet;
use super::clock::{
    LatchedReasons, PauseReasons, PauseRequest, apply_clock, compute_player_awaiting_system,
    process_pause_requests,
};
use super::decision::{recovery_system, undo_system};
use super::events::{ActionBlocked, PlayerTakeover, UndoCommand, UseFocus};
use super::focus::{Focus, PendingFocus, recover_focus_system, track_pending_focus_system};

/// 无回合时间线插件。
#[derive(Debug, Default)]
pub struct TimelinePlugin;

impl Plugin for TimelinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PauseReasons>()
            // 手动暂停的闩：与「本帧谁在停表」分开，免得从原因集合反推玩家意图
            .init_resource::<LatchedReasons>()
            .init_resource::<Focus>()
            .init_resource::<PendingFocus>()
            // 暂停断言：写方是 input（手动）、本域的等输入系统、combat 的威胁检测
            .add_message::<PauseRequest>()
            // 玩家自己动手：写方是 input / interaction，消费方是本域的撤销系统
            .add_message::<PlayerTakeover>()
            // Focus 换前摇：写方是 input（Shift + 决策键），消费方是本域
            .add_message::<UseFocus>()
            // 提示消息：写方是各声明系统，消费方是 HUD
            .add_message::<ActionBlocked>()
            .add_message::<UndoCommand>()
            // 撤销：undo_system 触发 EntityEvent，花钱的领域各自订阅退款
            // （打断的 Observer 住在 combat::formula：那是战斗判定，本域只提供数据）
            .add_systems(
                Update,
                (
                    // 暂停原因先算：后面的声明系统不需要知道冻结与否
                    compute_player_awaiting_system,
                    // Focus 请求只在本帧有效，慢一拍就会扣错账
                    track_pending_focus_system,
                    // 撤销：右键与「玩家动手了」在这一条系统里汇合
                    undo_system,
                    // 后摇恢复放最后：本帧执行器刚挂上的后摇不会被立刻摘掉
                    recovery_system,
                    recover_focus_system,
                )
                    .chain()
                    .in_set(TimelineSet),
            )
            .add_systems(
                Update,
                // 帧末结算钟表：这一帧所有系统看到的是同一个冻结状态
                (process_pause_requests, apply_clock)
                    .chain()
                    .in_set(ClockSet),
            );
    }
}
