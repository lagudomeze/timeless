//! 行动者的**决策槽**：谁能声明行动，由它一个人说了算。
//!
//! 三个阶段**直接写在槽里**，而不是靠「有没有行动实体 / 有没有后摇标记」推导：
//!
//! | 状态 | 含义 | 行动实体 | 可撤销 | 可打断 |
//! | :--- | :--- | :--- | :--- | :--- |
//! | `Empty` | 空闲，可以声明 | 无 | — | — |
//! | `Windup` | 前摇中 | 有 | ✓ | ✓ |
//! | `Recovery { until }` | 后摇中 | 无 | ✗ | ✗ |
//!
//! 转换只有五条路，每条都只有一个作者：
//!
//! ```text
//! Empty ──声明（8 个声明系统）──▶ Windup ──执行器收尾──▶ Recovery { until }
//!   ▲                              │                        │
//!   │                              ├── 撤销（undo_system）──┤
//!   │                              └── 打断（combat）───────┤
//!   └──────────────── recovery_system（now >= until）───────┘
//! ```
//!
//! 「谁在写槽」因此是穷举的、可审计的；不会出现「标记忘了摘」这类
//! 状态与时间戳打架的 bug。
//!
//! **声明的入口只有 [`FirstReady::first_ready`] 一个**：「槽必须是 `Empty`」这条判据与
//! 「被拒时告诉 HUD 为什么」都写在那里，各领域不再各抄一份。

use bevy::prelude::*;

use super::events::ActionBlocked;
use super::timing::ActionTiming;

/// 行动者的决策槽状态机。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub enum DecisionSlot {
    /// 空闲：可以声明行动
    #[default]
    Empty,
    /// 前摇中：行动实体还活着，随时可以反悔（撤销 / 打断）
    Windup,
    /// 后摇中：`until`（虚拟秒）之前不接受新决策
    Recovery {
        /// 重新可决策时刻（虚拟秒）
        until: f32,
    },
}

impl DecisionSlot {
    /// 现在能不能声明行动。
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// 执行器收尾：进入后摇。
    ///
    /// 后摇从**效果真的发生**那一刻起算，因此带位移 / 飞行的动作要把
    /// 「效果还要多久才发生」传进来（移动走到格中心、火球飞到落点）；
    /// 瞬间完成的动作传 `0`，只忙一个后摇。
    ///
    /// 传时长而不是「忙到哪个时刻」，是因为忙到的那一刻永远是
    /// `now + 这段时长`——少一次加法，也少一个"现在几点"的重复概念。
    pub fn recovering(timing: &ActionTiming, now: f32, effect_delay: f32) -> Self {
        Self::Recovery {
            until: now + timing.recovery.max(effect_delay),
        }
    }
}

/// 「这个查询项里带着行动者的决策槽」。
///
/// 为什么需要它：`QueryData` 的 item 是**元组**，Rust 没法从泛型元组里按类型取出
/// 某个分量。与其给 `Query<'w, 's, (…), F>` 写一堆带 GAT 的 impl，不如在**元组**
/// 上写几行——`Query::iter()` / `iter_mut()` 吐出来的就是元组本身。
///
/// 约定：**决策槽放在查询元组的最后一位**。这样只需要"每个元数一份"impl
/// （前面几个分量全是泛型，`&mut Stamina` / `Mut<Stamina>` 都能被吸收），
/// 不必为"槽在第几位"写组合数个版本。
pub trait HasDecisionSlot {
    /// 取出这一项里的决策槽。
    fn decision_slot(&self) -> &DecisionSlot;
}

impl<A> HasDecisionSlot for (A, &DecisionSlot) {
    fn decision_slot(&self) -> &DecisionSlot {
        self.1
    }
}

impl<A, B> HasDecisionSlot for (A, B, &DecisionSlot) {
    fn decision_slot(&self) -> &DecisionSlot {
        self.2
    }
}

impl<A, B, C> HasDecisionSlot for (A, B, C, &DecisionSlot) {
    fn decision_slot(&self) -> &DecisionSlot {
        self.3
    }
}

impl<A, B, C, D> HasDecisionSlot for (A, B, C, D, &DecisionSlot) {
    fn decision_slot(&self) -> &DecisionSlot {
        self.4
    }
}

impl<A, B, C, D, E> HasDecisionSlot for (A, B, C, D, E, &DecisionSlot) {
    fn decision_slot(&self) -> &DecisionSlot {
        self.5
    }
}

/// **占一个决策槽的唯一入口**：挑出那个现在能决策的行动者，挑不到就替 HUD
/// 记下原因（[`ActionBlocked::BUSY`]）。
///
/// 迭代器上的一个方法，所以调用点是主语在前的一句话：
///
/// ```text
/// let Some((player, cell, _)) = players.iter().first_ready(&mut blocked) else {
///     return;
/// };
/// ```
///
/// 哪个分量是决策槽由 [`HasDecisionSlot`] 回答（槽在末位），因此这里不需要
/// 调用方再给一个投影闭包。判据只有一份的好处是：以后要放宽
/// （比如"后摇里也允许排下一手"）或改提示（比如区分"前摇中"与"后摇中"），
/// 只改这一个方法。
pub trait FirstReady: Iterator + Sized {
    /// 第一个决策槽是 `Empty` 的项；一个都没有就报一条 `BUSY`。
    fn first_ready(self, blocked: &mut MessageWriter<ActionBlocked>) -> Option<Self::Item>
    where
        Self::Item: HasDecisionSlot,
    {
        let ready = self
            .into_iter()
            .find(|actor| actor.decision_slot().is_empty());
        if ready.is_none() {
            // 静默丢弃是最差的手感：告诉 HUD"现在还动不了"
            blocked.write(ActionBlocked::BUSY);
        }
        ready
    }
}

impl<I: Iterator> FirstReady for I {}

/// **记账的另一半**：把刚造出来的行动实体挂到行动者名下，并把行动者的决策槽
/// 推进 [`DecisionSlot::Windup`]。
///
/// 「行动是行动者的**子实体**」与「声明即占槽」都是时间线的规则，所以这两句
/// 只在这里写一遍；领域只负责造出那一刻的行动实体：
///
/// ```text
/// let action = commands.spawn_scene(move_action_scene(…)).id();
/// attach_action(&mut commands, player, action);
/// ```
///
/// 玩家路径与 AI 路径共用它——两边产出的行动实体完全一样，区别只在触发源。
pub fn attach_action(commands: &mut Commands, actor: Entity, action: Entity) {
    commands
        .entity(actor)
        .add_child(action)
        .insert(DecisionSlot::Windup);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 调度器的测试不该依赖任何具体载荷：自己造一个节奏。
    const TEST_TIMING: ActionTiming = ActionTiming::new(0.2, 0.3, 3);

    #[test]
    fn a_fresh_slot_is_empty() {
        assert_eq!(DecisionSlot::default(), DecisionSlot::Empty);
        assert!(DecisionSlot::Empty.is_empty());
        assert!(!DecisionSlot::Windup.is_empty());
        assert!(!DecisionSlot::Recovery { until: 1.0 }.is_empty());
    }

    /// 后摇取「一个后摇」与「效果还要多久」里更晚的那个。
    #[test]
    fn the_recovery_window_ends_at_the_later_of_effect_and_recovery() {
        // 效果比后摇晚（移动 / 火球）：忙到效果真的发生
        assert_eq!(
            DecisionSlot::recovering(&TEST_TIMING, 1.0, 0.4),
            DecisionSlot::Recovery { until: 1.4 }
        );
        // 效果瞬间完成（近战 / 招架）：只忙一个后摇
        assert_eq!(
            DecisionSlot::recovering(&TEST_TIMING, 1.0, 0.0),
            DecisionSlot::Recovery {
                until: 1.0 + TEST_TIMING.recovery
            }
        );
    }

    /// 挑人：空槽的中选；一个都没有就替 HUD 记一条 `BUSY`。
    ///
    /// 这条守着「声明的判据只有一份」——9 个声明系统原来各写一遍这段，
    /// 现在只有 `first_ready` 会写 `ActionBlocked`。
    #[test]
    fn first_ready_picks_the_first_empty_slot_or_reports_busy() {
        #[derive(Resource, Default)]
        struct Picked {
            actor: Option<Entity>,
            blocks: usize,
        }

        fn pick(
            actors: Query<(Entity, &DecisionSlot)>,
            mut blocked: MessageWriter<ActionBlocked>,
            mut out: ResMut<Picked>,
        ) {
            out.actor = actors
                .iter()
                .first_ready(&mut blocked)
                .map(|(entity, _)| entity);
        }

        fn count(mut requests: MessageReader<ActionBlocked>, mut out: ResMut<Picked>) {
            out.blocks += requests.read().count();
        }

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Picked>()
            .add_message::<ActionBlocked>()
            .add_systems(Update, (pick, count).chain());

        // 全是忙的：挑不到人，而且要报一条原因
        app.world_mut().spawn(DecisionSlot::Windup);
        app.update();
        let picked = app.world().resource::<Picked>();
        assert_eq!(picked.actor, None, "没人空着就挑不到");
        assert_eq!(picked.blocks, 1, "被拒时要替 HUD 记一条原因");

        // 来了个空槽的：挑中他，而且不再报原因
        let ready = app.world_mut().spawn(DecisionSlot::Empty).id();
        app.update();
        let picked = app.world().resource::<Picked>();
        assert_eq!(picked.actor, Some(ready), "空槽的人中选");
        assert_eq!(picked.blocks, 1, "挑到了就不该再报一条");
    }
}
