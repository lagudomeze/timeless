//! 时间线**对外**的跨领域契约：**别人怎么跟它说话**（[`PlayerTakeover`] / [`UseFocus`] /
//! [`UndoCommand`]，以及提示用的 [`ActionBlocked`]），**它怎么通知别人**（[`ActionCancelled`] /
//! [`DecisionReady`]）。
//!
//! 停表请求 [`PauseRequest`](super::clock::PauseRequest) 与它的原因集合是一个自成一体的
//! 话题（世界什么时候冻结），因此和它的系统一起住在 [`clock`](super::clock) 里。
//!
//! 批量、解耦的广播走 **Message**；「即时、针对具体实体」的响应走 **EntityEvent**——
//! 两者不混用（打断就是后者：它必须当场决定那条行动还在不在）。

use bevy::prelude::*;

/// 「玩家这一帧要自己动手 / 改主意」。
///
/// 写：[`crate::input`]（方向键 / 技能键）与 [`crate::interaction`]（左键点击）；
/// 消费：[`undo_system`](super::decision::undo_system)——它把玩家那条
/// **还没到点**的行动撤掉，好让同一帧稍后运行的声明系统抢到空的决策槽。
///
/// 为什么需要这条消息：声明系统看到"槽被占着"只会回一句
/// [`ActionBlocked`]，而玩家的真实意思是"我要改手"。而"这一帧玩家动手了没有"
/// 只有输入层知道——各领域的动作命令是下游派生的，晚一帧才出现，
/// 监听它们会把"刚由自己声明出来的行动"当成改主意撤掉。
/// 时间线因此不必认识 `MoveCommand` / `RollCommand` / `UseSelectedSkill` 这些词汇。
///
/// **它不是一个意图**（`Intent` 是决策层的结论，M23 才落地，见 `docs/timeline.md`）：这是输入层的**事实**
/// （玩家动手了），意图是决策层的**结论**（决定做什么）。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerTakeover;

/// 玩家要求「用 1 点 Focus 换前摇归零」（`Shift` + 决策键）。
///
/// 写：[`crate::input`]；消费：
/// [`track_pending_focus_system`](super::focus::track_pending_focus_system)——
/// 它只把这个请求记进 [`PendingFocus`](super::focus::PendingFocus)，
/// 玩家真的声明了行动才会扣费（光按 Shift 不花 Focus）。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UseFocus;

/// 「这次输入没被接受」。
///
/// 写：各声明系统（[`crate::movement::declare_move_system`] /
/// [`crate::combat::attack::declare_fireball_system`] 等）在玩家**还不能决策**或
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
/// 写：鼠标右键（`interaction`）/ 将来可能的 `Esc`；
/// 消费：[`undo_system`](super::decision::undo_system)——玩家自己动手
/// （[`PlayerTakeover`]）是撤销的另一个来源，两条在这里汇合。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UndoCommand;

/// 一条行动被撤销了（发给**行动实体**）。
///
/// 写：[`undo_system`](super::decision::undo_system)；
/// 消费：花钱的那个领域——目前住在 `combat::attack`（🚧 要改名 `combat::attack`）：火球退 2 收 2、近战收 1。
///
/// 用 `EntityEvent` 而不是广播 Message：撤销**一定**落在某一条具体行动上，
/// 而"退多少、收多少"只有看得到那条行动载荷的领域才知道——时间线不认识
/// [`FireballAction`](crate::combat::attack::FireballAction)。
///
/// ⚠️ **触发必须排在 `despawn` 之前**：Observer 是在命令应用阶段**当场**跑的，
/// 排在销毁之后就读不到行动实体身上的载荷了（表现为"退款静默丢失"）。
#[derive(EntityEvent, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionCancelled {
    /// 被撤销的行动实体（`EntityEvent` 的目标实体）
    pub entity: Entity,
    /// 被撤销行动的行动者
    pub actor: Entity,
}

/// 后摇结束、决策槽回到 `Empty`（发给**行动者**）。
///
/// 写：[`recovery_system`](super::decision::recovery_system)；
/// 消费：关心「又轮到它决策了」的领域——目前是 `combat::defense`（回 1 点精力）。
///
/// 用 `EntityEvent` 而不是让时间线直接改资源：资源归各自的领域管，
/// 时间线只宣布「槽空了」这件事。
#[derive(EntityEvent, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecisionReady {
    /// 重新拿到决策权的行动者
    pub entity: Entity,
}
