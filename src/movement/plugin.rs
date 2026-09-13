//! 移动领域插件：注册消息与「声明 → 执行 → 位移 → 到格」系统链。

use bevy::prelude::*;

use super::MovementSet;
use super::actions::{
    declare_jump_system, declare_move_system, declare_move_to_system, jump_action_executor_system,
    jump_motion_system, move_action_executor_system,
};
use super::events::{JumpCommand, MoveCommand, MoveToCommand};
use super::systems::{follow_terrain_system, move_entities_system};

/// 移动领域插件。
#[derive(Debug, Default)]
pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<MoveCommand>()
            .add_message::<MoveToCommand>()
            .add_message::<JumpCommand>()
            // 反射：`Cell` 是决策层的坐标，BRP / 调试面板想直接读它
            .register_type::<super::Cell>()
            .add_systems(
                Update,
                (
                    (
                        declare_move_system,
                        declare_move_to_system,
                        declare_jump_system,
                    ),
                    (move_action_executor_system, jump_action_executor_system),
                    // 先按速度位移，再贴地：贴地要在位移之后看到本帧的新位置
                    (move_entities_system, follow_terrain_system),
                    jump_motion_system,
                )
                    .chain()
                    .in_set(MovementSet),
            );
    }
}
