//! 时间线消息。

use bevy::prelude::*;

/// 玩家请求提交草案（仅 `require_commit = true` 时有效）。
///
/// 写：[`crate::input`]（Enter 键只翻译）；
/// 消费：[`commit_bridge_system`](super::systems::commit_bridge_system)——
/// 把草案 `Declared` 变成待执行的 `Pending`，世界重新开始流动。
#[derive(Message, Debug, Clone, Copy)]
pub struct ActionsCommitted;
