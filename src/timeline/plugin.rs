//! 时间线插件：注册资源、消息与「门控 → 提交 → 调度 → 后摇恢复」系统链。

use bevy::prelude::*;

use super::TimelineSet;
use super::events::ActionsCommitted;
use super::resources::{Timeline, TimelineConfig};
use super::systems::{
    commit_bridge_system, recovery_system, scheduler_system, timeline_gate_system,
};

/// 无回合时间线插件。
#[derive(Debug, Default)]
pub struct TimelinePlugin;

impl Plugin for TimelinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Timeline>()
            .init_resource::<TimelineConfig>()
            .add_message::<ActionsCommitted>()
            .add_systems(
                Update,
                (
                    // 门控先算：本帧该不该停表，后面所有领域都靠它
                    timeline_gate_system,
                    commit_bridge_system,
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
