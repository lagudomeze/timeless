//! 时间线消息与事件。
//!
//! 批量、解耦的广播走 **Message**；「即时、针对具体实体」的响应走 **EntityEvent**——
//! 两者不混用（打断就是后者：它必须当场决定那条行动还在不在）。

use bevy::prelude::*;

/// 停表 / 解冻请求。
///
/// **断言式**：谁这一帧还想让世界停着，就写一条 [`PauseRequest::Pause`]。
/// 不需要谁去"撤销"自己的原因——下一帧不再断言，原因自然消失
/// （[`PauseReasons`](super::resources::PauseReasons) 每帧重建）。
/// 「等玩家决策」与「威胁逼近」因此可以叠加、互不覆盖，也不会出现
/// "原因留在集合里没人摘"的幽灵冻结。
///
/// 写：[`crate::input`]（手动暂停，哪个键由输入域自己定）、时间线的
/// [`compute_player_awaiting_system`](super::systems::compute_player_awaiting_system)、
/// `combat::reaction::detect_threat_system`；
/// 消费：[`process_pause_requests`](super::systems::process_pause_requests)，
/// 再由 [`apply_clock`](super::systems::apply_clock) 落到 `Time<Virtual>`。
///
/// 原因只是**给人看的**（HUD 直接显示 `labels()`），所以是 `&'static str` 常量：
/// 加一个新原因不需要改任何枚举，只要一个常量加一个断言点，且每帧断言零分配。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseRequest {
    /// 这一帧仍然想停表，原因是 `reason`
    Pause(&'static str),
    /// 解冻：虚拟时间立刻流动，并清空已经收集到的原因
    Resume,
}

/// 「这一帧玩家表达了一个新意图」。
///
/// 写：[`crate::input`]（方向键 / 技能键）与 [`crate::interaction`]（左键点击）；
/// 消费：[`interrupt_system`](super::systems::interrupt_system)——它把玩家那条
/// **还没到点**的行动撤掉，好让同一帧稍后运行的声明系统抢到空的决策槽。
///
/// 为什么需要这条消息：声明系统看到"槽被占着"只会回一句
/// [`ActionBlocked`]，而玩家的真实意思是"我要改手"。而"这一帧有没有新意图"
/// 只有输入层知道——各领域的动作命令是下游派生的，晚一帧才出现，
/// 监听它们会把"刚由自己的意图声明出来的行动"当成新意图撤掉。
/// 时间线因此不必认识 `MoveCommand` / `RollCommand` / `UseSelectedSkill` 这些词汇。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerIntent;

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

/// 后摇结束、决策槽回到 `Empty`（发给**行动者**）。
///
/// 写：[`recovery_system`](super::systems::recovery_system)；
/// 消费：关心「又轮到它决策了」的领域——目前是 `combat::defense`（回 1 点精力）。
///
/// 用 `EntityEvent` 而不是让时间线直接改资源：资源归各自的领域管，
/// 时间线只宣布「槽空了」这件事。
#[derive(EntityEvent, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecisionReady {
    /// 重新拿到决策权的行动者
    pub entity: Entity,
}
