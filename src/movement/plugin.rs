//! 移动领域插件：注册消息与「声明 → 执行 → 位移 → 收尾」系统链。

use bevy::prelude::*;

use super::MovementSet;
use super::actions::{declare_move_system, move_action_executor_system};
use super::events::MoveCommand;
use super::systems::{move_entities_system, stop_on_round_end_system};

/// 移动领域插件。
#[derive(Debug, Default)]
pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<MoveCommand>().add_systems(
            Update,
            (
                declare_move_system,
                move_action_executor_system,
                move_entities_system,
                stop_on_round_end_system,
            )
                .chain()
                .in_set(MovementSet),
        );
    }
}
