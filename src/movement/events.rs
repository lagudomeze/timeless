//! 移动消息。

use bevy::prelude::*;

/// 移动指令：`axis` 是归一化的平面方向（无输入时不发消息）。
///
/// 写：[`crate::input`]（键盘只翻译，按下的那一次）；
/// 消费：[`declare_move_system`](super::actions::declare_move_system)——走一格。
#[derive(Message, Debug, Clone, Copy)]
pub struct MoveCommand {
    pub axis: Vec2,
}

/// 跳跃指令（空格）。
///
/// 写：[`crate::input`]（键盘只翻译）；
/// 消费：[`declare_jump_system`](super::actions::declare_jump_system)——只声明行动，
/// 到点后由跳跃执行器起步。
#[derive(Message, Debug, Clone, Copy)]
pub struct JumpCommand;
