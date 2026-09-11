//! 防御消息。
//!
//! 写：`input`（键盘只翻译）；消费：本域的声明系统。
//! 两者同属一个领域文件（AGENTS.md：消息定义与消费系统同域）。

use bevy::prelude::*;

/// 翻滚指令（`F`）。
///
/// 写：[`crate::input`]；消费：[`declare_roll_system`](super::actions::declare_roll_system)。
#[derive(Message, Debug, Clone, Copy)]
pub struct RollCommand;

/// 招架指令（`V`）。
///
/// 写：[`crate::input`]；消费：[`declare_parry_system`](super::actions::declare_parry_system)。
#[derive(Message, Debug, Clone, Copy)]
pub struct ParryCommand;
