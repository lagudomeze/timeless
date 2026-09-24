//! 移动消息。

use bevy::prelude::*;

use super::cell::Cell;

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

/// 移动到**指定格**（写：`interaction` 的左键点击；消费：
/// [`declare_move_to_system`](super::actions::declare_move_to_system)）。
///
/// 与 [`MoveCommand`] 的分工：那个是"朝屏幕方向走一格"（键盘），
/// 这个是"走到这一格"（鼠标）——**一次可以跨多格**，执行器沿直线走到目标格中心。
/// 沿途不做碰撞、不绕障碍：寻路要等体素碰撞（可行走性判定）落地再接，
/// 接口不变（到时候只是把"直线"换成"路径点序列"）。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveToCommand {
    pub cell: Cell,
}

/// 冲刺指令（`Shift` + 方向键）：朝该方向**冲两格**。
///
/// 写：[`crate::input`]；消费：[`declare_dash_system`](super::actions::declare_dash_system)。
///
/// **与 [`MoveCommand`] 同一个方向语义**（屏幕方向，由输入域换算成世界平面方向）：
/// 冲刺只是"同一方向、走两格、更快但也更贵"。
#[derive(Message, Debug, Clone, Copy)]
pub struct DashCommand {
    pub axis: Vec2,
}
