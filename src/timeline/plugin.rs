//! 时间线插件：注册状态、消息与调度系统链。

use bevy::prelude::*;

use super::TimelineSet;
use super::events::{ActionsCommitted, RoundEnded};
use super::resources::Timeline;
use super::systems::{
    commit_actions_system, end_round_system, pause_during_planning_system, scheduler_system,
};

/// We-Go 时间线插件。
#[derive(Debug, Default)]
pub struct TimelinePlugin;

impl Plugin for TimelinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Timeline>()
            .add_message::<ActionsCommitted>()
            .add_message::<RoundEnded>()
            .add_systems(
                Update,
                (
                    commit_actions_system,
                    end_round_system,
                    scheduler_system,
                    // 门控放最后：本帧结束时虚拟时间的状态与阶段一致
                    pause_during_planning_system,
                )
                    .chain()
                    .in_set(TimelineSet),
            );
    }
}
