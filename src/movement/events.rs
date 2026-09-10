//! 移动消息。

use bevy::prelude::*;

/// 移动指令：`axis` 是归一化的平面方向（无输入时为 `Vec2::ZERO`，表示停下）。
///
/// 写：[`crate::input`]（键盘只翻译）；
/// 消费：[`apply_move_command_system`](super::systems::apply_move_command_system)。
#[derive(Message, Debug, Clone, Copy)]
pub struct MoveCommand {
    pub axis: Vec2,
}
