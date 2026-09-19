//! 时间线系统：等谁决策、撤销、打断、后摇恢复、Focus 回复、钟表。
//!
//! 两条纪律：
//!
//! 1. **暂停只经 `PauseRequest`**：这一域里除了 [`apply_clock`] 谁也不碰
//!    `Time<Virtual>`，各领域因此不需要任何 `if paused` 分支；
//! 2. **收尾不集中**：执行器自己写 `ScheduledAction.execute_at` 的判据、
//!    自己销毁行动实体、自己把行动者推进 `Recovery`——时间线只提供数据与判定函数。

use bevy::prelude::*;

use super::components::{InputDriven, Uncancellable};
use super::decision::DecisionSlot;
use super::events::{
    ActionCancelled, DecisionReady, InterruptEvent, PauseRequest, PlayerIntent, UndoCommand,
    UseFocus,
};
use super::resources::{FOCUS_RECOVER_INTERVAL, Focus, FocusIntent, PauseReasons, SLOT_EMPTY};
use super::schedule::ScheduledAction;

/// 有 `InputDriven`（玩家）空着决策槽吗 → 世界该停下来等他。
///
/// 这是「无回合」里唯一的时间门控需求：敌人不等玩家，玩家一空闲，世界就停。
/// 没有 `InputDriven` 单位时（单测、组装之前）一律当作「不等输入」，避免把世界冻住。
///
/// **每帧断言**：还等着就再说一次。不需要谁去"撤销"——下一帧玩家动了，
/// 这里不再断言，原因自然从集合里消失（见 [`PauseRequest`]）。
pub fn compute_player_awaiting_system(
    actors: Query<&DecisionSlot, With<InputDriven>>,
    mut pause: MessageWriter<PauseRequest>,
) {
    if actors.iter().any(|slot| slot.is_empty()) {
        pause.write(PauseRequest::Pause(SLOT_EMPTY));
    }
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
/// 1. 这一帧玩家表达了新意图（见 [`PlayerIntent`]）；
/// 2. 玩家有一条未执行的行动（撤销系统自己会检查「还没到点 / 可取消 / 是玩家」）。
///
/// ⚠️ **只读输入层的意图**：`MoveCommand` / `RollCommand` / `UseSelectedSkill`
/// 这些是各领域的动作消息，时间线不该认识它们的词汇；而 `FireCommand` /
/// `MeleeCommand` 更是下游派生的，晚一帧才出现——监听它们会把"刚刚由自己的
/// 意图声明出来的行动"当成新意图撤掉（火球永远发不出去）。
pub fn interrupt_system(
    mut intents: MessageReader<PlayerIntent>,
    mut undo: MessageWriter<UndoCommand>,
) {
    if intents.read().next().is_none() {
        return; // 这一帧没人想做事：让当前意图继续
    }
    undo.write(UndoCommand);
}

/// 撤销：把玩家那条**还没到点**的行动撤掉——触发 [`ActionCancelled`]、销毁
/// 行动实体、清空决策槽。
///
/// 「还没到点」= `now < execute_at`。到点的行动这一帧就落地，撤不掉；
/// 没有可撤的行动时什么也不做（右键空放不报错，也不提示）。
///
/// 只撤**玩家**的行动：AI 的行动挂在同一条时间线上，但"谁能按键撤销"只有 PC
/// （和"谁能按键决策"是同一条约定：认 [`InputDriven`]）。
///
/// 挂着 [`Uncancellable`] 的行动直接跳过（跳跃那种"起跳就谁都别想插队"）。
pub fn undo_system(
    mut commands: Commands,
    mut requests: MessageReader<UndoCommand>,
    drivers: Query<(), With<InputDriven>>,
    actions: Query<(Entity, &ScheduledAction, &ChildOf), Without<Uncancellable>>,
    time: Res<Time<Virtual>>,
) {
    if requests.read().last().is_none() {
        return;
    }
    let now = time.elapsed_secs();
    for (entity, schedule, child_of) in &actions {
        // 行动者 = 父实体：归属由关系回答，不必在调度数据里再抄一份
        let actor = child_of.parent();
        if !schedule.pending(now) {
            continue; // 本帧就要执行，来不及撤
        }
        if drivers.get(actor).is_err() {
            continue; // AI 的行动只能被「打断」，不能被右键撤
        }
        // 先触发再销毁：Observer 当场跑，排在 despawn 之后就读不到载荷了
        commands.trigger(ActionCancelled { entity, actor });
        commands.entity(entity).despawn();
        // 行动者可能已经死了：往不存在的实体上写命令会让 Bevy 直接 panic
        if let Ok(mut actor_commands) = commands.get_entity(actor) {
            actor_commands.insert(DecisionSlot::Empty);
        }
        break; // 一次决策只有一条行动
    }
}

/// 后摇：`Recovery { until }` 到点就清空决策槽，并广播 [`DecisionReady`]。
///
/// 「又轮到它决策了」这件事只由时间线宣布；因此**谁能拿到它**（比如精力回复）
/// 由关心它的领域自己写 observer——时间线不反向依赖任何资源。
pub fn recovery_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    mut recovering: Query<(Entity, &mut DecisionSlot)>,
) {
    let now = time.elapsed_secs();
    for (entity, mut slot) in &mut recovering {
        // 只处理后摇：前摇归执行器与撤销 / 打断管，空闲没什么可恢复的
        let DecisionSlot::Recovery { until } = *slot else {
            continue;
        };
        if now < until {
            continue;
        }
        *slot = DecisionSlot::Empty;
        commands.trigger(DecisionReady { entity });
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

/// 暂停请求 → **本帧**的暂停原因集合。
///
/// 每帧重建：先清空，再按**发出顺序**处理这一帧的请求。
///
/// - [`PauseRequest::Pause`] 是**断言**："我还想让世界停着"——谁不停断言，
///   它的原因下一帧就不在了，不需要谁去撤销；
/// - [`PauseRequest::Resume`] 是**解冻**："清空此刻已经收集到的原因，让时间流动"。
///
/// 顺序因此有意义，而且是确定的：输入域（`Resume` 的来源）排在
/// [`TimelineSet`](super::TimelineSet) 之前，各领域的断言排在它之后——
/// 玩家手动解冻的那一帧，仍然成立的断言（比如"还等着你决策"）会照常加回来。
pub fn process_pause_requests(
    mut requests: MessageReader<PauseRequest>,
    mut reasons: ResMut<PauseReasons>,
) {
    reasons.clear();
    for request in requests.read() {
        match request {
            PauseRequest::Pause(reason) => reasons.insert(reason),
            PauseRequest::Resume => reasons.clear(),
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
    actions: Query<(Entity, &ScheduledAction, &ChildOf)>,
    mut commands: Commands,
) {
    let power = trigger.power;
    if power == 0 {
        return;
    }
    let target = trigger.entity;
    let source = trigger.source;
    let now = time.elapsed_secs();
    let Some((action, schedule, _)) = actions
        .iter()
        .find(|(_, schedule, child_of)| child_of.parent() == target && schedule.pending(now))
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
    use crate::timeline::{MANUAL, timing};
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
            .init_resource::<ManualLatch>()
            .init_resource::<Focus>()
            .init_resource::<FocusIntent>()
            .add_message::<PauseRequest>()
            .add_message::<UseFocus>()
            .add_message::<UndoCommand>()
            .init_resource::<Cancellations>()
            .add_observer(record_cancellation)
            .add_observer(interrupt_observer)
            .add_systems(
                Update,
                (
                    (
                        assert_manual,
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

    /// 测试用的「手动暂停开关」：真实实现里这个闩住在 `input` 域
    /// （空格切换它，然后每帧断言 [`MANUAL`]）。
    #[derive(Resource, Default)]
    struct ManualLatch(bool);

    /// 记下本帧收到过哪些撤销广播（真实的消费者住在花钱的领域里）。
    #[derive(Resource, Default)]
    struct Cancellations(Vec<(Entity, Entity)>);

    fn record_cancellation(cancelled: On<ActionCancelled>, mut recorded: ResMut<Cancellations>) {
        recorded.0.push((cancelled.entity, cancelled.actor));
    }

    fn assert_manual(latch: Res<ManualLatch>, mut pause: MessageWriter<PauseRequest>) {
        if latch.0 {
            pause.write(PauseRequest::Pause(MANUAL));
        }
    }

    /// 断言式的暂停：只要还在断言，世界就一直冻着；不再断言，下一帧就解冻。
    ///
    /// 旧实现是边沿触发的（靠 `Local<Option<bool>>` 记上一次的状态），
    /// 一旦有别人清空原因集合，它就再也不会把原因加回来——世界在"还等着玩家
    /// 决策"的状态下悄悄跑起来。
    #[test]
    fn a_reason_lasts_exactly_as_long_as_it_keeps_being_asserted() {
        let mut app = clock_app();
        let player = app
            .world_mut()
            .spawn((InputDriven, DecisionSlot::Empty))
            .id();

        app.update(); // 空槽 → 世界本来就冻着
        app.world_mut()
            .entity_mut(player)
            .insert(DecisionSlot::Windup);
        app.update();
        assert!(
            !app.world().resource::<Time<Virtual>>().is_paused(),
            "没有原因时世界应当流动"
        );

        app.world_mut().resource_mut::<ManualLatch>().0 = true;
        for _ in 0..6 {
            app.update();
            assert!(
                app.world().resource::<Time<Virtual>>().is_paused(),
                "只要还在断言，手动暂停就一直有效"
            );
        }

        app.world_mut().resource_mut::<ManualLatch>().0 = false;
        app.update();
        assert!(
            !app.world().resource::<Time<Virtual>>().is_paused(),
            "不再断言，原因下一帧就该消失"
        );
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
        app.world_mut().resource_mut::<ManualLatch>().0 = true;
        app.update(); // manual + slot_empty
        assert_eq!(
            app.world().resource::<PauseReasons>().labels(),
            vec![MANUAL, SLOT_EMPTY],
            "两个原因可以同时挂着"
        );

        app.world_mut()
            .entity_mut(player)
            .insert(DecisionSlot::Windup);
        app.update();
        assert!(
            app.world().resource::<Time<Virtual>>().is_paused(),
            "槽不空了，但手动暂停还挂着"
        );
        assert_eq!(
            app.world().resource::<PauseReasons>().labels(),
            vec![MANUAL],
            "不再成立的原因应当自己消失，而不是等人来摘"
        );

        app.world_mut().resource_mut::<ManualLatch>().0 = false;
        app.update();
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    }

    /// `Resume` 的语义：清空**此刻已经收集到**的原因。
    ///
    /// 顺序因此是有意义的，而且是确定的——输入域（`Resume` 的来源）排在
    /// [`TimelineSet`] 之前，各领域的断言排在它之后。
    #[test]
    fn a_resume_only_clears_what_was_asserted_before_it() {
        let mut app = clock_app();
        app.world_mut().resource_mut::<ManualLatch>().0 = true;

        // 解冻请求先到，同一帧里仍然成立的断言随后加回来
        app.world_mut().write_message(PauseRequest::Resume);
        app.update();
        assert!(
            app.world().resource::<Time<Virtual>>().is_paused(),
            "解冻只清掉此刻已收集的原因；同一帧里仍然成立的断言要照常加回来"
        );

        // 断言停掉之后，解冻才真的生效
        app.world_mut().resource_mut::<ManualLatch>().0 = false;
        app.world_mut().write_message(PauseRequest::Resume);
        app.update();
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    }

    /// 后摇到点：`Recovery { until }` → `Empty`，而且**只**碰后摇。
    #[test]
    fn recovery_clears_the_slot_only_after_the_window_ends() {
        fn slot_of(app: &App, entity: Entity) -> Option<DecisionSlot> {
            app.world().get::<DecisionSlot>(entity).copied()
        }

        let mut app = clock_app();
        let actor = app
            .world_mut()
            .spawn(DecisionSlot::Recovery { until: 0.35 })
            .id();
        let winding_up = app.world_mut().spawn(DecisionSlot::Windup).id();

        // 「到点」是唯一的清槽条件：虚拟时间没走到 until 就一直留着
        let mut frames = 0;
        while app.world().resource::<Time<Virtual>>().elapsed_secs() < 0.35 {
            assert_eq!(
                slot_of(&app, actor),
                Some(DecisionSlot::Recovery { until: 0.35 }),
                "后摇没到点不该清槽"
            );
            app.update();
            frames += 1;
            assert!(frames < 20, "虚拟时间没有推进，后摇永远不会结束");
        }
        assert_eq!(
            slot_of(&app, actor),
            Some(DecisionSlot::Empty),
            "后摇到点应当清空决策槽"
        );
        assert_eq!(
            slot_of(&app, winding_up),
            Some(DecisionSlot::Windup),
            "前摇归执行器与撤销 / 打断管，后摇系统不该碰它"
        );
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

    /// 撤销：广播 [`ActionCancelled`]、销毁行动实体、清空决策槽。
    ///
    /// 退多少归花钱的领域（见 `combat::skills` 的退款 Observer），这里只保证
    /// **广播打在行动实体上、而且发生在销毁之前**——排在销毁之后的话，
    /// Observer 已经读不到载荷，退款会静默丢失。
    #[test]
    fn undo_broadcasts_the_cancellation_before_despawning() {
        let mut app = clock_app();
        let player = app
            .world_mut()
            .spawn((InputDriven, DecisionSlot::Windup))
            .id();
        let action = app
            .world_mut()
            .spawn((
                ChildOf(player),
                ScheduledAction::declared_at(timing::SHOOT, 0.0),
            ))
            .id();

        app.world_mut().write_message(UndoCommand);
        app.update();

        assert_eq!(
            app.world().resource::<Cancellations>().0,
            vec![(action, player)],
            "撤销要广播在行动实体上，并带上行动者"
        );
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

    /// 挂着 [`Uncancellable`] 的行动：连右键也撤不掉（跳跃那种"起跳不插队"）。
    #[test]
    fn an_uncancellable_action_survives_the_undo() {
        let mut app = clock_app();
        let player = app
            .world_mut()
            .spawn((InputDriven, DecisionSlot::Windup))
            .id();
        let action = app
            .world_mut()
            .spawn((
                ChildOf(player),
                ScheduledAction::declared_at(timing::JUMP, 0.0),
                Uncancellable,
            ))
            .id();

        app.world_mut().write_message(UndoCommand);
        app.update();

        assert!(
            app.world().get_entity(action).is_ok(),
            "不可撤销的行动不该被右键打掉"
        );
        assert!(
            app.world().resource::<Cancellations>().0.is_empty(),
            "没撤掉就不该广播撤销"
        );
        assert_eq!(
            app.world().get::<DecisionSlot>(player).copied(),
            Some(DecisionSlot::Windup),
            "行动还在，决策槽也还占着"
        );
    }

    /// 到点的行动撤不掉：`execute_at <= now` 就是"这一手已经出去了"。
    #[test]
    fn an_action_that_already_came_due_cannot_be_undone() {
        let mut app = clock_app();
        let player = app
            .world_mut()
            .spawn((InputDriven, DecisionSlot::Windup))
            .id();
        let action = app
            .world_mut()
            .spawn((
                ChildOf(player),
                ScheduledAction::declared_at(timing::MOVE, 0.0),
            ))
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
        let target = app.world_mut().spawn(DecisionSlot::Windup).id();
        let action = app
            .world_mut()
            .spawn((
                ChildOf(target),
                ScheduledAction::declared_at(timing::MELEE, 0.0),
            ))
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
        let target = app.world_mut().spawn(DecisionSlot::Windup).id();
        let action = app
            .world_mut()
            .spawn((
                ChildOf(target),
                ScheduledAction::declared_at(timing::MOVE, 0.0),
            ))
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

    /// 打断只认**输入层的意图**这一条消息：时间线不认识各领域的命令词汇。
    #[derive(Resource, Default)]
    struct Undos(usize);

    fn count_undos(mut requests: MessageReader<UndoCommand>, mut undos: ResMut<Undos>) {
        undos.0 += requests.read().count();
    }

    #[test]
    fn only_a_player_intent_asks_for_an_undo() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Undos>()
            .add_message::<PlayerIntent>()
            .add_message::<UndoCommand>()
            .add_systems(Update, (interrupt_system, count_undos).chain());

        app.update();
        assert_eq!(app.world().resource::<Undos>().0, 0, "没人表态就不该撤销");

        app.world_mut().write_message(PlayerIntent);
        app.update();
        assert_eq!(
            app.world().resource::<Undos>().0,
            1,
            "一句玩家意图就够时间线撤掉那条还没到点的行动"
        );
    }
}
