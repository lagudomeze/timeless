//! 时间线消息。

use bevy::prelude::*;

/// 玩家请求暂停 / 继续（空格）。
///
/// 写：[`crate::input`]（空格只翻译）；消费：[`pause_toggle_system`](super::systems::pause_toggle_system)。
/// **空格只表示暂停**，不触发任何行动。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TogglePause;

/// 玩家请求切换反应窗口松紧（`F2`）。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CycleReactionWindow;

/// 「这次输入没被接受」。
///
/// 写：各声明系统（[`crate::movement::declare_move_system`] /
/// [`crate::combat::skills::declare_fireball_system`] 等）在玩家**还不能决策**或**资源不够**时；
/// 消费：HUD（提示玩家为什么没动）。
///
/// 无回合模型里忙碌的单位不接受新声明，输入会被**静默丢弃**——那是手感最差的一类反馈，
/// 所以显式发一条消息让界面说清楚。
///
/// 为什么住在 `timeline` 而不是表现层：它讲的是「谁可以决策」（[`super::Ready`]），
/// 而 `movement` / `combat` 本来就依赖时间线；放进表现层会让下层**反向依赖**表现层。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionBlocked {
    pub reason: BlockReason,
}

impl ActionBlocked {
    /// 还没恢复 `Ready`（前摇 / 后摇 / 位移中）。
    pub const BUSY: Self = Self {
        reason: BlockReason::Busy,
    };
    /// 精力不够。
    pub const NO_ENERGY: Self = Self {
        reason: BlockReason::NotEnoughEnergy,
    };
}

/// 输入被拒的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockReason {
    /// 还没恢复 `Ready`
    Busy,
    /// 精力不够
    NotEnoughEnergy,
}

/// 玩家请求撤销**最近一条尚未结算的玩家行动**（草案或已提交、还没到点）。
///
/// 写：鼠标右键（`interaction`）/ 将来可能的 `Esc`；
/// 消费：[`undo_system`](super::systems::undo_system)。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UndoCommand;

/// 一条玩家行动被撤销了（写：[`undo_system`](super::systems::undo_system)；
/// 消费：资源所属的领域，目前是 `combat::defense` 退精力）。
///
/// 为什么要广播而不是直接退：撤销要退资源、而资源归各自领域管——时间线只负责
/// "把这条行动撤掉"，退多少、退给谁由资源的拥有者决定。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionCancelled {
    /// 被撤销行动的行动者
    pub actor: Entity,
    /// 需要退还的资源量（来自行动实体上的 [`ActionCost`](super::ActionCost)）
    pub refund: u32,
    /// 取消本身的代价（来自 [`CancelCost`](super::CancelCost)；0 = 免费）
    pub penalty: u32,
}
