//! 交互域的消息：鼠标点击（写：`input` 的指针系统；消费：本域的
//! [`pointer_command_system`](super::pointer::pointer_command_system)）。
//!
//! 输入层只把"哪个键被按了"翻成消息，**不知道游戏世界**；把它解释成
//! "走哪一格 / 打谁 / 撤销" 是本域的事（拾取要相机与地形）。

use bevy::prelude::*;

/// 一次指针点击。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerCommand {
    /// 左键：对目标动手——点在单位上就是技能，点在空地就是移动。
    Primary,
    /// 右键：撤销当前预操作。
    Secondary,
}
