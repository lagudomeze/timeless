//! 时间线系统：等谁决策、撤销、打断、后摇恢复、Focus 回复、钟表。
//!
//! 两条纪律：
//!
//! 1. **暂停只经 `PauseRequest`**：这一域里除了 [`apply_clock`] 谁也不碰
//!    `Time<Virtual>`，各领域因此不需要任何 `if paused` 分支；
//! 2. **收尾不集中**：执行器自己写 `ScheduledAction.execute_at` 的判据、
//!    自己销毁行动实体、自己挂 [`Busy`]——时间线只提供数据与判定函数。

use bevy::prelude::*;

use crate::combat::defense::{STAMINA_REGEN_PER_DECISION, Stamina};

use super::components::{Busy, Cancellable, DecisionSlot, InputDriven, ScheduledAction};
use super::events::{
    ActionCancelled, InterruptEvent, PauseRequest, TogglePause, UndoCommand, UseFocus,
};
use super::resources::{
    FOCUS_RECOVER_INTERVAL, Focus, FocusIntent, MANUAL, ManualPause, PauseReasons, SLOT_EMPTY,
};

/// 边沿触发的小工具：状态**翻转**时才写一条暂停请求。
///
/// 每帧都写同一个原因有两个坏处：白白分配字符串，以及让「谁在停世界」淹没在
/// 噪声里（排查暂停问题时最想看的恰恰是第一条）。
fn request_on_edge(
    last: &mut Option<bool>,
    wanted: bool,
    reason: &str,
    pause: &mut MessageWriter<PauseRequest>,
) {
    if *last == Some(wanted) {
        return;
    }
    *last = Some(wanted);
    if wanted {
        pause.write(PauseRequest::Pause(reason.to_string()));
    } else {
        pause.write(PauseRequest::Resume(reason.to_string()));
    }
}

/// 空格 → 手动暂停开关（只写原因，不碰时钟）。
///
/// 旧实现直接把 `Time<Virtual>` 反着设一次，而「等玩家输入」的门控下一帧又会
/// 把它设回来——净效果是**手动暂停只前进一帧**。现在两个原因各自独立：
/// 手动暂停一直挂在集合里，直到玩家再按一次空格。
pub fn compute_manual_pause(
    mut requests: MessageReader<TogglePause>,
    mut manual: ResMut<ManualPause>,
    mut pause: MessageWriter<PauseRequest>,
    mut last: Local<Option<bool>>,
) {
    if requests.read().last().is_some() {
        manual.0 = !manual.0;
    }
    request_on_edge(&mut last, manual.0, MANUAL, &mut pause);
}

/// 有 `InputDriven`（玩家）空着决策槽吗 → 世界该停下来等他。
///
/// 这是「无回合」里唯一的时间门控需求：敌人不等玩家，玩家一空闲，世界就停。
/// 没有 `InputDriven` 单位时（单测、组装之前）一律当作「不等输入」，避免把世界冻住。
pub fn compute_player_awaiting_system(
    actors: Query<&DecisionSlot, With<InputDriven>>,
    mut pause: MessageWriter<PauseRequest>,
    mut last: Local<Option<bool>>,
) {
    let awaiting = actors.iter().any(|slot| *slot == DecisionSlot::Empty);
    request_on_edge(&mut last, awaiting, SLOT_EMPTY, &mut pause);
}

/// 玩家想不想用 Focus 换前摇（本帧有效）：`Shift` + 决策键 → [`UseFocus`]。
pub fn track_focus_intent_system(
    mut requests: MessageReader<UseFocus>,
    mut intent: ResMut<FocusIntent>,
) {
    intent.0 = requests.read().last().is_some();
}

/// 打断：本帧只要出现**新意图**，就把玩家那条可取消的未执行行动撤掉。
///
/// 无回合模型里"随时可以改主意"就靠它。判定是纯谓词，两条同时满足才触发：
///
/// 1. 这一帧有玩家直接产生的意图（多种意图同帧时取**最后一次**读，与声明系统一致）；
/// 2. 玩家有一条未执行的行动（撤销系统自己会检查「还没到点 / 可取消 / 是玩家」）。
///
/// ⚠️ **只读玩家直接产生的输入层消息**：`FireCommand` / `MeleeCommand` 是
/// `UseSelectedSkill`（或热键）派生的下游消息，晚一帧才出现——监听它们会把
/// "刚刚由自己的意图声明出来的行动"当成新意图撤掉（火球永远发不出去）。
pub fn interrupt_system(
    mut moves: MessageReader<crate::movement::MoveCommand>,
    mut moves_to: MessageReader<crate::movement::MoveToCommand>,
    mut jumps: MessageReader<crate::movement::JumpCommand>,
    mut uses: MessageReader<crate::combat::skills::UseSelectedSkill>,
    mut rolls: MessageReader<crate::combat::defense::RollCommand>,
    mut parries: MessageReader<crate::combat::defense::ParryCommand>,
    mut undo: MessageWriter<UndoCommand>,
) {
    let wanted = moves.read().count()
        + moves_to.read().count()
        + jumps.read().count()
        + uses.read().count()
        + rolls.read().count()
        + parries.read().count();
    if wanted == 0 {
        return; // 这一帧没人想做事：让当前意图继续
    }
    undo.write(UndoCommand);
}

/// 撤销：把玩家那条**还没到点**的行动撤掉——销毁行动实体、清空决策槽，
/// 并广播 [`ActionCancelled`] 让资源所属的领域退还花费。
///
/// 「还没到点」= `now < execute_at`。到点的行动这一帧就落地，撤不掉；
/// 没有可撤的行动时什么也不做（右键空放不报错，也不提示）。
///
/// 只撤**玩家**的行动：AI 的行动挂在同一条时间线上，但"谁能按键撤销"只有 PC
/// （和"谁能按键决策"是同一条约定：认 [`InputDriven`]）。
pub fn undo_system(
    mut commands: Commands,
    mut requests: MessageReader<UndoCommand>,
    drivers: Query<(), With<InputDriven>>,
    actions: Query<(Entity, &ScheduledAction, Option<&Cancellable>)>,
    time: Res<Time<Virtual>>,
    mut cancelled: MessageWriter<ActionCancelled>,
) {
    if requests.read().last().is_none() {
        return;
    }
    let now = time.elapsed_secs();
    for (entity, schedule, cancellable) in &actions {
        if !schedule.pending(now) {
            continue; // 本帧就要执行，来不及撤
        }
        // 行动自己说不给撤（跳跃那种）：连右键也撤不掉
        let cancellable = cancellable.copied().unwrap_or_default();
        if cancellable == Cancellable::Never {
            continue;
        }
        if drivers.get(schedule.actor).is_err() {
            continue; // AI 的行动只能被「打断」，不能被右键撤
        }
        commands.entity(entity).despawn();
        // 行动者可能已经死了：往不存在的实体上写命令会让 Bevy 直接 panic
        if let Ok(mut actor) = commands.get_entity(schedule.actor) {
            actor.insert(DecisionSlot::Empty);
        }
        cancelled.write(ActionCancelled {
            actor: schedule.actor,
            refund: cancellable.refund(),
            penalty: cancellable.penalty(),
        });
        break; // 一次决策只有一条行动
    }
}

/// 后摇：到点清空决策槽、摘掉 [`Busy`]，并回一点精力。
///
/// 恢复决策槽是「又轮到它决策了」，因此这里也是精力的自然回复点
/// （取代旧模型的「每回合 +1」——无回合没有回合）。
pub fn recovery_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    mut recovering: Query<(Entity, &Busy, &mut DecisionSlot, Option<&mut Stamina>)>,
) {
    let now = time.elapsed_secs();
    for (entity, busy, mut slot, stamina) in &mut recovering {
        if now < busy.until {
            continue;
        }
        if let Some(mut stamina) = stamina {
            stamina.regen(STAMINA_REGEN_PER_DECISION);
        }
        *slot = DecisionSlot::Empty;
        commands.entity(entity).remove::<Busy>();
    }
}

/// Focus 回复：每 [`FOCUS_RECOVER_INTERVAL`] 虚拟秒回 1 点。
///
/// 走到 `Time<Virtual>` 上，因此**冻结时不回复**：暂停不是"白送资源"的时间。
pub fn recover_focus_system(
    mut focus: ResMut<Focus>,
    time: Res<Time<Virtual>>,
    mut timer: Local<f32>,
) {
    *timer += time.delta_secs();
    while *timer >= FOCUS_RECOVER_INTERVAL {
        *timer -= FOCUS_RECOVER_INTERVAL;
        focus.recover();
    }
}

/// 暂停请求 → 暂停原因集合（谁想停世界就往里加一条，不想要了撤掉）。
pub fn process_pause_requests(
    mut requests: MessageReader<PauseRequest>,
    mut reasons: ResMut<PauseReasons>,
) {
    for request in requests.read() {
        match request {
            PauseRequest::Pause(reason) => reasons.insert(reason.clone()),
            PauseRequest::Resume(reason) => reasons.remove(reason),
        }
    }
}

/// **唯一**写 `Time<Virtual>` 的地方：原因集合非空就冻表，否则解冻。
///
/// 它排在帧末的 [`ClockSet`](super::ClockSet)，因此影响的是**下一帧**——
/// 这一帧里所有系统看到的都是同一个时钟状态，不会出现"半帧冻、半帧不冻"。
pub fn apply_clock(reasons: Res<PauseReasons>, mut time: ResMut<Time<Virtual>>) {
    if reasons.is_frozen() {
        if !time.is_paused() {
            time.pause();
            debug!("⏸ 世界冻结：{:?}", reasons.labels());
        }
    } else if time.is_paused() {
        time.unpause();
        debug!("▶ 世界继续");
    }
}

/// 3d5：三个五面骰之和（3..=15）。打断对抗用它给双方各加一点运气。
pub fn roll_3d5() -> i32 {
    (0..3).map(|_| rand::random_range(1..=5)).sum()
}

/// 打断 Observer：命中打过来时，对目标那条**还没到点**的行动做一次掷骰对抗。
///
/// ```text
/// 攻方 = power + 3 + 3d5
/// 守方 = interrupt_resist + 3 + 3d5
/// 攻方 >= 守方 → 这条行动被销毁，目标立刻拿回决策槽
/// ```
///
/// 规则细节：
/// - 只打断 `execute_at > now` 的行动——**本帧到点的已经落地**，打不断；
/// - `power == 0` 直接返回（有力度才谈对抗）；
/// - 打断是"抹掉还没发生的事"，因此不退款、不还精力：那一手白费了。
pub fn interrupt_observer(
    trigger: On<InterruptEvent>,
    time: Res<Time<Virtual>>,
    actions: Query<(Entity, &ScheduledAction)>,
    mut commands: Commands,
) {
    let power = trigger.power;
    if power == 0 {
        return;
    }
    let target = trigger.entity;
    let source = trigger.source;
    let now = time.elapsed_secs();
    let Some((action, schedule)) = actions
        .iter()
        .find(|(_, schedule)| schedule.actor == target && schedule.pending(now))
    else {
        return; // 来不及：这一手已经落地，或者本来就没事可打断
    };

    let attack = power + 3 + roll_3d5();
    let defense = schedule.interrupt_resist + 3 + roll_3d5();
    if attack < defense {
        debug!("⚖ 打断失败：{source:?} 对 {target:?}（{attack} < {defense}）");
        return;
    }

    info!("⚡ 打断成功：{source:?} 打掉了 {target:?} 的行动（{attack} >= {defense}）");
    commands.entity(action).despawn();
    if let Ok(mut actor) = commands.get_entity(target) {
        actor.insert(DecisionSlot::Empty);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::timing;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    /// 最小 App：只装时间线要用的东西，手动步进 100ms/帧。
    fn clock_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .init_resource::<PauseReasons>()
            .init_resource::<ManualPause>()
            .init_resource::<Focus>()
            .init_resource::<FocusIntent>()
            .add_message::<TogglePause>()
            .add_message::<PauseRequest>()
            .add_message::<UseFocus>()
            .add_message::<UndoCommand>()
            .add_message::<ActionCancelled>()
            .add_observer(interrupt_observer)
            .add_systems(
                Update,
                (
                    (
                        compute_manual_pause,
                        compute_player_awaiting_system,
                        track_focus_intent_system,
                        undo_system,
                        recovery_system,
                        recover_focus_system,
                    )
                        .chain(),
                    // 钟表在最后：顺序与生产流水线（TimelineSet → ClockSet）一致
                    (process_pause_requests, apply_clock).chain(),
                )
                    .chain(),
            );
        app
    }

    /// 手动暂停必须**一直有效**，而不是只前进一帧。
    ///
    /// 旧实现里门控每帧都按「玩家就绪」暂停、`pause_toggle_system` 又反着设一次，
    /// 净效果是空格只让世界多走一帧。
    #[test]
    fn manual_pause_stays_until_the_player_toggles_it_back() {
        let mut app = clock_app();
        let player = app
            .world_mut()
            .spawn((InputDriven, DecisionSlot::Empty))
            .id();

        app.update(); // 空槽 → 世界本来就冻着
        app.world_mut()
            .entity_mut(player)
            .insert(DecisionSlot::Filled);
        app.update();
        assert!(
            !app.world().resource::<Time<Virtual>>().is_paused(),
            "没有原因时世界应当流动"
        );

        // 空格 → 手动暂停
        app.world_mut().write_message(TogglePause);
        app.update();
        assert!(app.world().resource::<Time<Virtual>>().is_paused());

        // 再跑几帧：世界仍然冻着（旧实现这里就解冻了）
        for _ in 0..5 {
            app.update();
            assert!(
                app.world().resource::<Time<Virtual>>().is_paused(),
                "手动暂停应当一直有效"
            );
        }

        // 再按一次空格 → 解冻
        app.world_mut().write_message(TogglePause);
        app.update();
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    }

    /// 空决策槽是另一个独立原因：它消失时手动暂停仍然有效。
    #[test]
    fn pause_reasons_do_not_override_each_other() {
        let mut app = clock_app();
        let player = app
            .world_mut()
            .spawn((InputDriven, DecisionSlot::Empty))
            .id();

        app.update(); // slot_empty
        app.world_mut().write_message(TogglePause);
        app.update(); // manual + slot_empty

        app.world_mut()
            .entity_mut(player)
            .insert(DecisionSlot::Filled);
        app.update();
        assert!(
            app.world().resource::<Time<Virtual>>().is_paused(),
            "槽不空了，但手动暂停还挂着"
        );

        app.world_mut().write_message(TogglePause);
        app.update();
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    }

    /// Focus 只走虚拟时间：冻住时一分都不回。
    #[test]
    fn focus_recovers_only_while_the_world_runs() {
        let mut app = clock_app();
        app.world_mut().resource_mut::<Focus>().current = 0;

        app.update();
        for _ in 0..30 {
            app.update();
        }
        assert_eq!(
            app.world().resource::<Focus>().current,
            0,
            "冻结时不该回复 Focus"
        );

        app.world_mut().resource_mut::<Time<Virtual>>().unpause();
        for _ in 0..101 {
            app.update();
        }
        assert_eq!(
            app.world().resource::<Focus>().current,
            1,
            "世界走了 10 秒就该回 1 点"
        );
    }

    /// 撤销：销毁行动实体、清空决策槽、按 `Cancellable` 广播退款。
    #[test]
    fn undo_clears_the_slot_and_reports_the_cost() {
        let mut app = clock_app();
        let player = app
            .world_mut()
            .spawn((InputDriven, DecisionSlot::Filled))
            .id();
        let action = app
            .world_mut()
            .spawn((
                ScheduledAction::declared_at(player, timing::SHOOT, 0.0),
                Cancellable::Cost {
                    refund: 2,
                    penalty: 1,
                },
            ))
            .id();

        app.world_mut().write_message(UndoCommand);
        app.update();

        assert!(
            app.world().get_entity(action).is_err(),
            "撤销要把行动实体销毁"
        );
        assert_eq!(
            app.world().get::<DecisionSlot>(player).copied(),
            Some(DecisionSlot::Empty),
            "撤销之后应当立刻能重新决策"
        );
    }

    /// 到点的行动撤不掉：`execute_at <= now` 就是"这一手已经出去了"。
    #[test]
    fn an_action_that_already_came_due_cannot_be_undone() {
        let mut app = clock_app();
        let player = app
            .world_mut()
            .spawn((InputDriven, DecisionSlot::Filled))
            .id();
        let action = app
            .world_mut()
            .spawn(ScheduledAction::declared_at(player, timing::MOVE, 0.0))
            .id();

        // 世界走了 1 秒：0.15s 的前摇早就过了
        app.world_mut().resource_mut::<Time<Virtual>>().unpause();
        for _ in 0..12 {
            app.update();
        }
        app.world_mut().write_message(UndoCommand);
        app.update();

        assert!(
            app.world().get_entity(action).is_ok(),
            "已经到点的行动不该被撤销"
        );
    }

    /// 打断：掷骰对抗赢了就销毁行动并清空决策槽；`power == 0` 不做对抗。
    #[test]
    fn interrupt_despawns_a_pending_action_and_frees_the_slot() {
        let mut app = clock_app();
        let target = app.world_mut().spawn(DecisionSlot::Filled).id();
        let action = app
            .world_mut()
            .spawn(ScheduledAction::declared_at(target, timing::MELEE, 0.0))
            .id();
        let source = app.world_mut().spawn_empty().id();

        // 力度 0：连对抗都不做
        app.world_mut().trigger(InterruptEvent {
            entity: target,
            source,
            power: 0,
        });
        app.world_mut().flush(); // Observer 里的命令要落到世界才看得到
        assert!(app.world().get_entity(action).is_ok());

        // 力度 100：3d5 的差值最大 12，必赢
        app.world_mut().trigger(InterruptEvent {
            entity: target,
            source,
            power: 100,
        });
        app.world_mut().flush();
        assert!(
            app.world().get_entity(action).is_err(),
            "被打断的行动应当消失"
        );
        assert_eq!(
            app.world().get::<DecisionSlot>(target).copied(),
            Some(DecisionSlot::Empty),
            "被打断的人应当立刻拿回决策槽"
        );
    }

    /// 本帧到点的行动已经落地，打不断。
    #[test]
    fn an_action_that_came_due_this_frame_survives_an_interrupt() {
        let mut app = clock_app();
        let target = app.world_mut().spawn(DecisionSlot::Filled).id();
        let action = app
            .world_mut()
            .spawn(ScheduledAction::declared_at(target, timing::MOVE, 0.0))
            .id();
        let source = app.world_mut().spawn_empty().id();

        app.world_mut().resource_mut::<Time<Virtual>>().unpause();
        for _ in 0..12 {
            app.update();
        }
        app.world_mut().trigger(InterruptEvent {
            entity: target,
            source,
            power: 100,
        });
        app.world_mut().flush();
        assert!(
            app.world().get_entity(action).is_ok(),
            "已经到点的行动打不断（它已经出去了）"
        );
    }

    /// 玩家想用 Focus 换前摇时，意图只在本帧有效。
    #[test]
    fn focus_intent_lasts_exactly_one_frame() {
        let mut app = clock_app();
        app.world_mut().write_message(UseFocus);
        app.update();
        assert!(app.world().resource::<FocusIntent>().0);

        app.update();
        assert!(
            !app.world().resource::<FocusIntent>().0,
            "上一帧的意图不该延续到下一帧"
        );
    }
}
