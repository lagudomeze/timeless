//! 时间线消息与事件。
//!
//! 批量、解耦的广播走 **Message**；「即时、针对具体实体」的响应走 **EntityEvent**——
//! 两者不混用（打断就是后者：它必须当场决定那条行动还在不在）。

use bevy::prelude::*;

/// 玩家请求暂停 / 继续（空格）。
///
/// 写：[`crate::input`]（空格只翻译）；消费：
/// [`compute_manual_pause`](super::systems::compute_manual_pause)。
/// **空格只表示暂停**，不触发任何行动；真正的停表发生在 `apply_clock`。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TogglePause;

/// 停表 / 解冻请求：按**原因**加减，而不是直接开关时钟。
///
/// 写：时间线自己的 `compute_*` 系统与 `combat::reaction::detect_threat_system`；
/// 消费：[`process_pause_requests`](super::systems::process_pause_requests)
/// （累积成 [`PauseReasons`](super::resources::PauseReasons)），
/// 再由 [`apply_clock`](super::systems::apply_clock) 落到 `Time<Virtual>`。
///
/// 原因用字符串是为了让「谁停了世界」一目了然（HUD 直接显示 `labels()`），
/// 而且新原因不需要改任何枚举：加一个常量 + 一个 `compute_*` 系统即可。
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub enum PauseRequest {
    /// 加上一个暂停原因
    Pause(String),
    /// 撤掉一个暂停原因
    Resume(String),
}

/// 玩家要求「用 1 点 Focus 换前摇归零」（`Shift` + 决策键）。
///
/// 写：[`crate::input`]；消费：
/// [`track_focus_intent_system`](super::systems::track_focus_intent_system)——
/// 它只把这个意图记进 [`FocusIntent`](super::resources::FocusIntent)，
/// 玩家真的声明了行动才会扣费（光按 Shift 不花 Focus）。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UseFocus;

/// 「这次输入没被接受」。
///
/// 写：各声明系统（[`crate::movement::declare_move_system`] /
/// [`crate::combat::skills::declare_fireball_system`] 等）在玩家**还不能决策**或
/// **资源不够**时；消费：HUD（提示玩家为什么没动）。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionBlocked {
    pub reason: BlockReason,
}

impl ActionBlocked {
    /// 决策槽还不是 `Empty`（前摇 / 后摇 / 位移中）。
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
    /// 决策槽被占着（前摇 / 后摇：见 [`DecisionSlot`](super::DecisionSlot)）
    Busy,
    /// 精力不够
    NotEnoughEnergy,
}

/// 玩家请求撤销**那条还没到点的玩家行动**。
///
/// 写：鼠标右键（`interaction`）/ 打断系统 / 将来可能的 `Esc`；
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
    /// 需要退还的资源量（来自行动上的 [`Cancellable`](super::Cancellable)）
    pub refund: u32,
    /// 取消本身的代价（来自同一个 [`Cancellable`](super::Cancellable)；0 = 免费）
    pub penalty: u32,
}

/// 一次打断：**命中打过来**，把目标那条还没到点的行动打掉。
///
/// 用 `EntityEvent` 而不是 Message：它针对具体实体、必须即时生效
/// （Observer 里当场决定那条行动还在不在），且没有「批量广播」的语义。
///
/// 写：命中的结算系统（[`crate::combat::formula::apply_physical_hits_system`]）；
/// 消费：[`interrupt_observer`](super::systems::interrupt_observer)。
///
/// 规则：只打断 `execute_at > now` 的行动（本帧到点的已经落地，打不断）；
/// `power == 0` 直接返回，不做对抗。
#[derive(EntityEvent, Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptEvent {
    /// 被打断的目标（`EntityEvent` 的目标实体）
    pub entity: Entity,
    /// 打断源（攻击实体 / 技能来源，日志与复盘用）
    pub source: Entity,
    /// 打断力度
    pub power: i32,
}
