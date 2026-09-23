//! 时间线插件：注册资源、消息与观察者，并声明两段系统链。
//!
//! ```text
//! TimelineSet（帧中）：断言暂停原因（PC 没决定）→ 记 Focus 请求 → 等待的声明 / 执行
//!                     → 撤销（右键与「玩家动手了」在此汇合）→ 后摇恢复 → Focus 回复
//! ```
//!
//! 本域**不认识按键**：手动暂停的"按哪个键、按一下是暂停还是继续"由
//! [`crate::input`] 决定，它只发一条 [`PauseRequest`](crate::clock::PauseRequest)。
//!
//! **冻结设施不在本域**：原因集合 / 时钟写入住在 [`crate::clock`]（通用，
//! 不认识决策槽）。本域只贡献一条断言——[`compute_player_awaiting_system`]，
//! 因为只有本域认识决策槽。

use bevy::prelude::*;

use super::TimelineSet;
use super::decision::{compute_player_awaiting_system, recovery_system, undo_system};
use super::events::{ActionBlocked, PlayerTakeover, UndoCommand, UseFocus};
use super::focus::{PendingFocus, recover_focus_system, track_pending_focus_system};
use super::wait::{
    WaitCommand, declare_wait_system, register_wait_ability_system, wait_executor_system,
};

/// 无回合时间线插件。
#[derive(Debug, Default)]
pub struct TimelinePlugin;

impl Plugin for TimelinePlugin {
    fn build(&self, app: &mut App) {
        app
            // 「等待」动作：写方是 input（空格），消费方是本域
            .add_message::<WaitCommand>()
            // 目录必须存在才能把「等待」交上去（与 combat / movement 同一约定）
            .add_systems(Startup, register_wait_ability_system)
            .init_resource::<PendingFocus>()
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
                    // 等待：声明（占槽）与执行（到点收尾）都在本域
                    (declare_wait_system, wait_executor_system),
                    // 后摇恢复放最后：本帧执行器刚挂上的后摇不会被立刻摘掉
                    recovery_system,
                    recover_focus_system,
                )
                    .chain()
                    .in_set(TimelineSet),
            );
    }
}
