//! 时间线消息。

use bevy::prelude::*;

/// 玩家请求提交本轮行动。
///
/// 写：[`crate::input`]（Enter 键只翻译）；
/// 消费：[`commit_actions_system`](super::systems::commit_actions_system)——
/// 把本轮所有 `Declared` 行动变成 `Pending` 并开始推进。
#[derive(Message, Debug, Clone, Copy)]
pub struct ActionsCommitted;

/// 一轮推进结束，虚拟时间已冻结、回到规划阶段。
///
/// 写：[`end_round_system`](super::systems::end_round_system)；
/// 消费：各领域的「本轮收尾」（如 [`crate::movement`] 让单位停下、AI 走一轮冷却）。
#[derive(Message, Debug, Clone, Copy)]
pub struct RoundEnded {
    /// 刚结束的轮次编号（从 1 开始）
    pub round: u32,
}
