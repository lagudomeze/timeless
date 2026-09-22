//! 「等待」：**占住决策槽一小段时间**，等价于玩家说"我要停一下"。
//!
//! ## 为什么它是一条技能，而不是暂停层的特例
//!
//! 无回合模型里「什么都不做」也是一种决定。用既有的动作机制表达它，
//! 于是不需要给 [`crate::clock`] 加任何特例：
//!
//! ```text
//! 玩家空闲（槽空）→ awaiting 断言 → 世界冻着等他
//!   按空格 → 声明一条 Wait（槽被占）
//!          → awaiting 不再断言 → 世界**继续跑**
//!          → 1s 后行动落地 → 槽进后摇 → 回到 Idle → 又等他
//! ```
//!
//! 关键在第二步：**槽被占住，`awaiting` 自然消失**——不是谁去把暂停"解开"，
//! 而是"他还欠着一个决定"这件事暂时不成立了。这正是断言式暂停的好处。
//!
//! ## 它为什么住在 `timeline`
//!
//! 行动 = 独立实体 + 载荷 + 执行器，而"占住槽"是决策槽的语义。
//! 载荷与执行器都只碰 [`DecisionSlot`] / [`ActionOf`]，不认识任何机制领域
//! （移动 / 战斗 / 防御），所以它属于调度层本身——就像"能不能决策"属于它一样。
//!
//! 它**不挂 [`Threatens`](crate::combat::reaction::Threatens)**：站着不动不构成威胁。

use bevy::prelude::*;

use crate::skills::AbilityId;

use super::decision::{DecisionSlot, FirstReady, InputDriven, Intent, Target};
use super::ownership::ActionOf;
use super::schedule::{ActionTiming, ScheduledAction};

/// 等待多久（虚拟秒）。
///
/// 选 1s 是手感而非机制：短到不耽误事，长到"我看一眼再决定"够用。
pub const WAIT_SECONDS: f32 = 1.0;

/// 等待的节奏：**后摇就是等待时长**，没有前摇。
///
/// 「效果"就是等完这一秒，所以落地时刻 = 声明时刻（前摇 0），
/// 而重新可决策的时刻 = 落地 + 后摇 = 1s 后。
pub const WAIT_TIMING: ActionTiming = ActionTiming::new(0.0, WAIT_SECONDS, 99);

/// 等待载荷。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WaitAction;

/// 等待行动的场景工厂（载荷 + 节奏 + 调度 + 归属）。
pub fn wait_action_scene(
    timing: ActionTiming,
    schedule: ScheduledAction,
    actor: Entity,
) -> impl Scene {
    bsn! {
        ActionOf({actor})
        WaitAction
        template_value(timing)
        template_value(schedule)
    }
}

/// 声明等待：占住决策槽 [`WAIT_SECONDS`] 秒。
///
/// 与其它 9 个声明系统同形（`first_ready` 挑人 → 物化 → 写槽），
/// 区别只在它不需要目标、也不消耗任何资源。
pub fn declare_wait_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    mut requests: MessageReader<WaitCommand>,
    mut blocked: MessageWriter<super::events::ActionBlocked>,
    players: Query<(Entity, &DecisionSlot), With<InputDriven>>,
) {
    if requests.read().last().is_none() {
        return;
    }
    // 忙（前摇 / 后摇）或没有玩家时，first_ready 会替 HUD 记下原因
    let Some((player, _)) = players.iter().first_ready(&mut blocked) else {
        return;
    };
    let now = time.elapsed_secs();
    commands.spawn_scene(wait_action_scene(
        WAIT_TIMING,
        ScheduledAction::declared_at(WAIT_TIMING, now),
        player,
    ));
    commands.entity(player).insert(DecisionSlot::declared(
        Intent {
            ability: AbilityId::Wait,
            target: Target::None,
        },
        &WAIT_TIMING,
        now,
    ));
}

/// 执行等待：**什么都不做**，只把行动者推进后摇（= 忙完这一秒）。
///
/// 与其它执行器同形：到点 → 销毁行动实体 → 写 [`DecisionSlot::recovering`]。
/// 「效果」就是这一秒本身，所以不需要 `effect_delay`。
pub fn wait_executor_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    actions: Query<(
        Entity,
        &ActionTiming,
        &WaitAction,
        &ScheduledAction,
        &ActionOf,
    )>,
) {
    let now = time.elapsed_secs();
    for (entity, timing, _wait, schedule, action_of) in &actions {
        if !schedule.due(now) {
            continue;
        }
        let actor = action_of.actor();
        commands.entity(entity).despawn();
        if let Ok(mut actor_commands) = commands.get_entity(actor) {
            actor_commands.insert(DecisionSlot::recovering(timing, schedule, 0.0));
        }
    }
}

/// 请求等待（写：[`crate::input`] 的空格；消费：本域的声明系统）。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitCommand;

/// 等待域的技能定义（交给 [`crate::skills`] 的目录）。
pub const WAIT_ABILITY: crate::skills::AbilityDef = crate::skills::AbilityDef {
    id: AbilityId::Wait,
    category: crate::skills::AbilityCategory::Posture,
    timing: WAIT_TIMING,
    targeting: crate::skills::TargetSelector::SelfOnly,
    cost: 0,
    requirements: &[],
    combat: crate::skills::CombatTags::COMMITTED,
    power: 0,
};

/// 开局把等待的定义交上去（与 `movement` / `combat` 同一套约定）。
pub fn register_wait_ability_system(
    mut registrations: MessageWriter<crate::skills::RegisterAbility>,
) {
    registrations.write(crate::skills::RegisterAbility(WAIT_ABILITY));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::test_support::timeline_app;

    /// 等待的节奏：**没有前摇**（声明即落地），忙满 [`WAIT_SECONDS`]。
    #[test]
    fn waiting_costs_only_the_recovery_and_has_no_windup() {
        assert_eq!(WAIT_TIMING.windup, 0.0, "等待不需要前摇");
        assert_eq!(
            WAIT_TIMING.recovery, WAIT_SECONDS,
            "后摇就是等待时长——落地之后再忙完这一秒"
        );
        assert_eq!(WAIT_TIMING.total(), WAIT_SECONDS, "声明到重新可决策正好 1s");
    }

    /// 声明等待：行动者**当场**被推进后摇（`until = now + total`），槽不再空闲。
    ///
    /// 这条守着的正是它存在的理由：槽被占住 → 等玩家的那条断言自然消失
    /// → 世界继续跑。
    #[test]
    fn declaring_a_wait_takes_the_slot_immediately() {
        let mut app = timeline_app();
        app.add_message::<WaitCommand>()
            .add_message::<crate::timeline::ActionBlocked>()
            .add_systems(Update, (declare_wait_system, wait_executor_system).chain());
        let player = app
            .world_mut()
            .spawn((InputDriven, DecisionSlot::Idle { intent: None }))
            .id();

        app.world_mut().write_message(WaitCommand);
        app.update();

        let slot = *app.world().get::<DecisionSlot>(player).unwrap();
        assert!(
            !slot.is_idle(),
            "等待也要占住槽（这正是 awaiting 消失的原因），实际 {slot:?}"
        );
        assert!(slot.ready(), "声明之后就算「已经决定了」");
    }

    /// 忙的人按空格声明不了（`first_ready` 会替他记一条原因）。
    #[test]
    fn a_busy_actor_cannot_wait() {
        let mut app = timeline_app();
        app.add_message::<WaitCommand>()
            .add_message::<crate::timeline::ActionBlocked>()
            .add_systems(Update, declare_wait_system);
        let player = app
            .world_mut()
            .spawn((InputDriven, DecisionSlot::Executing { until: 999.0 }))
            .id();

        app.world_mut().write_message(WaitCommand);
        app.update();

        assert_eq!(
            *app.world().get::<DecisionSlot>(player).unwrap(),
            DecisionSlot::Executing { until: 999.0 },
            "忙的时候不该被等待顶掉"
        );
    }
}
