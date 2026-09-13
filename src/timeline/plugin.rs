//! 时间线插件：注册资源、消息与「门控 → 提交 → 调度 → 后摇恢复」系统链。

use bevy::prelude::*;

use super::TimelineSet;
use super::events::{
    ActionBlocked, ActionCancelled, CycleReactionWindow, TogglePause, UndoCommand,
};
use super::resources::{Timeline, TimelineConfig};
use super::systems::{
    commit_bridge_system, cycle_reaction_window_system, interrupt_system, pause_toggle_system,
    recovery_system, scheduler_system, timeline_gate_system, undo_system,
};

/// 无回合时间线插件。
#[derive(Debug, Default)]
pub struct TimelinePlugin;

impl Plugin for TimelinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Timeline>()
            .init_resource::<TimelineConfig>()
            // 空格 = 暂停 / 继续；F2 = 反应窗口松紧
            .add_message::<TogglePause>()
            .add_message::<CycleReactionWindow>()
            // 提示消息：写方是各声明系统，消费方是 HUD
            .add_message::<ActionBlocked>()
            .add_message::<UndoCommand>()
            // 撤销的退款广播：写方是本域，消费方是资源所属领域（combat::defense）
            .add_message::<ActionCancelled>()
            .add_systems(
                Update,
                (
                    // 门控先算：本帧该不该停表，后面所有领域都靠它
                    timeline_gate_system,
                    // 空格 / F2 是纯时间与配置控制，紧跟在门控之后
                    (pause_toggle_system, cycle_reaction_window_system),
                    // 打断：本帧有新意图就先撤掉旧的（可取消的）行动
                    interrupt_system,
                    commit_bridge_system,
                    // 撤销插在提交/调度之前：撤销的消息只在玩家按右键的那一帧存在
                    undo_system,
                    scheduler_system,
                    // 后摇恢复放最后：本帧执行器刚挂上的后摇不会被立刻摘掉
                    // （`ready_at` 严格大于落地时刻，恢复最早也要下一帧）
                    recovery_system,
                )
                    .chain()
                    .in_set(TimelineSet),
            );
    }
}
