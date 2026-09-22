//! # timeline — 无回合时间线（能决策就决策）
//!
//! 没有阶段、没有轮次：**每个单位决策槽空闲下来就能再决策**，
//! 节奏由每个动作自带的前摇 + 后摇决定（[`ActionTiming`]）。
//!
//! ```text
//! [Idle] ──声明──▶ [Executing { until } + 行动实体] ──到点──▶ 执行器落地
//!    ▲                                                              │
//!    └───────────── recovery_system（now >= until）─────────────────┘
//! ```
//!
//! 槽里装的是**决策**（[`DecisionSlot`]），不是**阶段**——「这一手到哪一步了」
//! 由行动实体自己的 `execute_at` 回答，槽不重复表达一遍：
//!
//! | 状态 | 含义 | 行动实体 |
//! | :--- | :--- | :--- |
//! | `Idle { intent: None }` | 空闲，可以声明 | 无 |
//! | `Idle { intent: Some(..) }` | 已决定、还没排进时间轴 | 无 |
//! | `Executing { until }` | 在时间轴上，忙到 `until` | 有（前摇）/ 已销毁（后摇） |
//!
//! 本域只做调度，**不感知载荷**：行动 = 独立实体，实体上只有调度数据
//! [`ScheduledAction`] + 载荷 + 可选的 [`Uncancellable`]，归属用自定义关系
//! [`ActionOf`] / [`Actions`]（**不是** `ChildOf`：行动没有 `Transform`，
//! 归属是纯逻辑，见 `docs/relations.md`）。执行器住在各自的领域里
//! （[`crate::movement`] 与 [`crate::combat`]），
//! 新增动作时调度器一行不改；收尾也**不集中**——执行器自己销毁行动实体、
//! 自己把行动者推进后摇。
//!
//! ## 冻结：原因集合，而不是一个布尔
//!
//! ```text
//! frozen ⟺ 本帧的 PauseReasons 非空
//! ```
//!
//! 请求有**两种时序**：各领域写 [`PauseRequest::Pause`]（**每帧断言**，下一帧不再
//! 断言原因就自然消失）；玩家的手动暂停写 [`PauseRequest::Toggle`]（**翻转开关**，
//! 翻一次之后由闩住的原因每帧自己续上）。按哪个键、按一下是暂停还是继续，是
//! [`crate::input`] 的事——它只发一条 `Toggle`，**不读** [`PauseReasons`]
//! （那里面混着别人的原因，反推会把"恢复"误判成"暂停"）。
//! **唯一**写 `Time<Virtual>` 的地方是帧末 [`ClockSet`] 里的 [`process_pause_requests`]。
//!
//! Bevy 每帧把虚拟时间拷进通用 `Time`，因此位移、投射物、`Lifetime`、后摇计时
//! 全部自动停表，各领域不需要任何 `if paused` 分支。
//!
//! 决策按**格子**、命中按**真实距离**，格边长见
//! [`crate::movement::CELL_SIZE`]（格尺度是移动领域的真相，本域只是读它）。
//!
//! ## 文件地图
//!
//! 每个文件回答一个问题：
//!
//! | 文件 | 回答什么问题 |
//! | :--- | :--- |
//! | [`decision`] | 谁能决策（槽 + 入口）；这一手怎么被撤掉 / 收尾（undo / recovery） |
//! | [`ownership`] | 这一手是谁的（`ActionOf` / `Actions`）；「行动者没了，行动也跟着没」 |
//! | [`schedule`] | 行动实体身上与时间有关的数据：这一手何时落地、这类动作的节奏、能不能撤 |
//! | [`wait`] | 「等待」动作：占住槽一小段时间 = 我要停一下（玩家空格的绑定） |
//! | [`focus`] | Focus 一族（⚠️ 不是时间线的概念，见该文件顶部的警告） |
//!
//! 冻结世界的设施**不在本域**了：它是通用的（不认识决策槽），住在 [`crate::clock`]。
//! | [`events`] | 时间线**对外**的跨领域契约：别人怎么跟它说话、它怎么通知别人 |
//! | [`plugin`] | 接线：注册资源 / 消息 / 观察者，声明两段系统链 |

use bevy::prelude::*;

pub mod decision;
pub mod events;
pub mod focus;
pub mod ownership;
pub mod plugin;
pub mod schedule;
pub mod wait;

pub use decision::{
    DecisionSlot, FirstReady, HasDecisionSlot, InputDriven, Intent, Target,
    compute_player_awaiting_system,
};
pub use decision::{recovery_system, undo_system};
pub use events::{
    ActionBlocked, ActionCancelled, BlockReason, DecisionReady, PlayerTakeover, UndoCommand,
    UseFocus,
};
pub use focus::{FOCUS_MAX, FOCUS_RECOVER_INTERVAL, Focus, PendingFocus};
pub use focus::{recover_focus_system, track_pending_focus_system};
pub use ownership::{ActionOf, Actions};
pub use plugin::TimelinePlugin;
pub use schedule::{ActionTiming, ScheduledAction, Uncancellable};
pub use wait::{
    WAIT_ABILITY, WAIT_SECONDS, WAIT_TIMING, WaitAction, WaitCommand, declare_wait_system,
    register_wait_ability_system, wait_action_scene, wait_executor_system,
};

/// 时间线在 `Update` 中的系统集（排在输入之后、AI 与执行器之前）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimelineSet;

#[cfg(test)]
pub(crate) mod test_support {
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    use super::*;
    use crate::clock::{ManualLatch, assert_manual};

    /// 记下本帧收到过哪些撤销广播（真实的消费者住在花钱的领域里）。
    #[derive(Resource, Default)]
    pub(crate) struct Cancellations(pub(crate) Vec<(Entity, Entity)>);

    pub(crate) fn record_cancellation(
        cancelled: On<ActionCancelled>,
        mut recorded: ResMut<Cancellations>,
    ) {
        recorded.0.push((cancelled.entity, cancelled.actor));
    }

    /// 只装时间线自己的东西：手动步进 100ms/帧 + 本域全部资源、消息、系统与观察者。
    /// 各文件的单测复用它，保证跑的是真实的域内顺序。
    pub fn timeline_app() -> App {
        let mut app = App::new();
        // 行动实体用 `spawn_scene` 建，所以需要 `AssetServer` + `ScenePlugin`
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .add_plugins(bevy::scene::ScenePlugin)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .init_resource::<crate::clock::PauseReasons>()
            .init_resource::<crate::clock::ManualPause>()
            .init_resource::<ManualLatch>()
            .init_resource::<Focus>()
            .init_resource::<PendingFocus>()
            .add_message::<crate::clock::PauseRequest>()
            .add_message::<PlayerTakeover>()
            .add_message::<UseFocus>()
            .add_message::<UndoCommand>()
            .init_resource::<Cancellations>()
            .add_observer(record_cancellation)
            .add_systems(
                Update,
                (
                    (
                        assert_manual,
                        compute_player_awaiting_system,
                        track_pending_focus_system,
                        undo_system,
                        recovery_system,
                        recover_focus_system,
                    )
                        .chain(),
                    // 钟表在最后：顺序与生产流水线（TimelineSet → ClockSet）一致
                    crate::clock::process_pause_requests,
                )
                    .chain(),
            );
        app
    }
}
