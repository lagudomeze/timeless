//! 玩家控制消息
use bevy::prelude::*;

/// 移动指令：`axis` 为归一化平面方向（无方向时为 `Vec2::ZERO`，表示停下）。
/// 由 `player_move_input_system` 写入、`apply_move_command_system` 消费。
#[derive(Message, Debug, Clone, Copy)]
pub struct MoveCommand {
    pub axis: Vec2,
}
