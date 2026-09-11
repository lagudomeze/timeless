//! 移动领域插件：注册消息与「声明 → 执行 → 位移 → 到格」系统链。

use bevy::prelude::*;

use super::MovementSet;
use super::actions::{
    declare_jump_system, declare_move_system, jump_action_executor_system, jump_motion_system,
    move_action_executor_system,
};
use super::events::{JumpCommand, MoveCommand};
use super::systems::move_entities_system;

/// 移动领域插件。
#[derive(Debug, Default)]
pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<MoveCommand>()
            .add_message::<JumpCommand>()
            .add_systems(
                Update,
                (
                    (declare_move_system, declare_jump_system),
                    (move_action_executor_system, jump_action_executor_system),
                    move_entities_system,
                    jump_motion_system,
                )
                    .chain()
                    .in_set(MovementSet),
            );
    }
}
