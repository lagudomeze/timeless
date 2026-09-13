//! 时间线系统：玩家输入门控、提交桥、调度、后摇恢复。
//!
//! 无回合模型下没有全局阶段，节奏由每个动作自己的前摇 + 后摇决定；
//! 这里只负责四件事：**什么时候停表**、**草案何时升为待执行**、
//! **行动何时到点**、**后摇何时结束**。

use bevy::prelude::*;

use crate::combat::Faction;
use crate::combat::defense::{STAMINA_REGEN_PER_DECISION, Stamina};

use super::components::{
    ActionCost, BusyRecovery, CancelCost, Committed, Declared, Pending, Ready, ScheduledAction,
    Uncancellable,
};
use super::events::{ActionCancelled, CycleReactionWindow, TogglePause, UndoCommand};
use super::resources::{ReactionWindow, Timeline, TimelineConfig};

/// **唯一的暂停点**：玩家就绪、还在地上、且还没做完决定时冻结虚拟时间。
///
/// 各领域因此不需要任何 `if paused` 分支——移动、后摇、投射物生命周期自动停表。
/// **空中不冻结**：跳跃是不可中断的弹道；若在落地前因为「玩家又就绪了」而停表，
/// 单位会僵在半空。等它落地再等输入。
pub fn timeline_gate_system(
    config: Res<TimelineConfig>,
    mut timeline: ResMut<Timeline>,
    mut time: ResMut<Time<Virtual>>,
    players: Query<(Entity, &Faction), With<Ready>>,
    factions: Query<&Faction>,
    airborne: Query<(), With<crate::movement::Jumping>>,
    threats: Query<&crate::combat::CollisionTarget, With<ScheduledAction>>,
) {
    let player_ready = players
        .iter()
        .any(|(_, faction)| *faction == Faction::Player);
    // 没有玩家实体（单测 / 组装之前）一律当作「不等输入」，避免把世界冻住
    let has_player = factions.iter().any(|faction| *faction == Faction::Player);
    let someone_airborne = !airborne.is_empty();

    // 威胁：有攻击正在前摇、且瞄准玩家 → 按反应窗口的松紧决定要不要停。
    // 「瞄准玩家」= 那条未结算的攻击挂着 `CollisionTarget(玩家)`。
    let threatened = threats.iter().any(|target| {
        factions
            .get(target.0)
            .is_ok_and(|faction| *faction == Faction::Player)
    });
    let reaction_pause = match config.reaction {
        ReactionWindow::Off => false,
        ReactionWindow::Loose => threatened,
        // 严格模式：只有玩家**能反应**（就绪且没在空中）时才停
        ReactionWindow::Strict => threatened && player_ready && !someone_airborne,
    };
    let waiting = has_player && (!someone_airborne) && (player_ready || reaction_pause);
    timeline.set_waiting_for_input(waiting);

    if waiting {
        if !time.is_paused() {
            time.pause();
            debug!("⏸ 等玩家决策：虚拟时间冻结");
        }
    } else if time.is_paused() {
        time.unpause();
    }
}

/// 提交桥：把声明升为待执行——**声明即生效，没有"确认"这一步**。
///
/// 所有载荷声明出来都是 [`Declared`] 状态；这里把它们统一升为 `Pending`，
/// 于是「按下 / 点击 → 到点执行」是一条直路。玩家想反悔不用等确认，
/// 直接用新意图**打断**（见 [`undo_system`] 与各声明系统的取消逻辑）。
pub fn commit_bridge_system(
    mut commands: Commands,
    mut timeline: ResMut<Timeline>,
    declared: Query<Entity, With<Declared>>,
) {
    for entity in &declared {
        commands.entity(entity).remove::<Declared>().insert(Pending);
    }
    timeline.set_draft(None);
}

/// 空格 → 暂停 / 继续（手动停表；不影响任何行动的声明）。
pub fn pause_toggle_system(
    mut requests: MessageReader<TogglePause>,
    mut time: ResMut<Time<Virtual>>,
) {
    if requests.read().next().is_none() {
        return;
    }
    if time.is_paused() {
        time.unpause();
    } else {
        time.pause();
    }
}

/// `F2` → 循环反应窗口松紧（Loose → Strict → Off）。
pub fn cycle_reaction_window_system(
    mut requests: MessageReader<CycleReactionWindow>,
    mut config: ResMut<TimelineConfig>,
) {
    if requests.read().next().is_none() {
        return;
    }
    config.reaction = config.reaction.next();
    info!("reaction window: {}", config.reaction.label());
}

/// 声明动作时的统一收尾：移除就绪标记 + 把草案记到时间线上。
///
/// 载荷领域（移动 / 技能 / AI）在 spawn 行动实体后调用它，避免各自重复这段逻辑。
pub fn begin_action(
    commands: &mut Commands,
    timeline: &mut Timeline,
    actor: Entity,
    draft: Entity,
) {
    commands.entity(actor).remove::<Ready>();
    timeline.set_draft(Some(draft));
}

/// 调度：到点的行动标记为 `Committed`，交给执行器。
///
/// 调度器只读 [`ScheduledAction`]，不认识任何载荷。时间基准统一用
/// `Time<Virtual>`——它正是 `execute_at` 的来源（暂停时两者一起停）。
pub fn scheduler_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    actions: Query<(Entity, &ScheduledAction), With<Pending>>,
) {
    let now = now.elapsed_secs();
    for (entity, action) in &actions {
        if now >= action.execute_at {
            commands
                .entity(entity)
                .remove::<Pending>()
                .insert(Committed);
        }
    }
}

/// 后摇：到点后恢复 [`Ready`]，并回一点精力。执行器在落地效果时挂上 [`BusyRecovery`]。
///
/// 恢复 `Ready` 是「又轮到它决策了」，因此这里也是精力的自然回复点
/// （取代旧模型的「每回合 +1」——无回合没有回合）。
pub fn recovery_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    recovering: Query<(Entity, &BusyRecovery, Option<&mut Stamina>)>,
) {
    let now = now.elapsed_secs();
    for (entity, recovery, stamina) in recovering {
        if now < recovery.ready_at {
            continue;
        }
        if let Some(mut stamina) = stamina {
            stamina.regen(STAMINA_REGEN_PER_DECISION);
        }
        commands
            .entity(entity)
            .remove::<BusyRecovery>()
            .insert(Ready);
    }
}

/// 执行器收尾（各载荷领域共用）：摘掉「已到点」标记 + 销毁行动实体 + 给行动者挂后摇窗口。
///
/// **必须摘掉 [`Committed`]**：它表示「本帧等待执行器处理」；若不摘，
/// 所有 `With<Committed>` 的执行器每帧都会重复触发同一个动作
/// （表现为跳跃无限上升、技能连发）。
///
/// `Ready` 的恢复交给 [`recovery_system`]——`ready_at` 严格大于落地时刻，
/// 因此恢复最早发生在下一帧。
pub fn end_action(
    commands: &mut Commands,
    action: Entity,
    actor: Entity,
    schedule: &ScheduledAction,
    executed_at: f32,
) {
    end_action_until(commands, action, actor, schedule, executed_at, executed_at);
}

/// 同 [`end_action`]，但可以指定「行动者至少要忙到什么时候」。
///
/// **带位移的动作必须用它**（移动 / 翻滚）：后摇 0.10s 而走一格要 0.40s 时，
/// 只按后摇恢复 `Ready` 会让玩家在**滑行途中**拿到决策权，而决策层的
/// [`Cell`](crate::movement::Cell) 还是旧格（它只在到位时更新）——下一手声明
/// 就会用旧格当起点，表现为「反复按 A/D 时单位掉头 / 回弹」。
pub fn end_action_until(
    commands: &mut Commands,
    action: Entity,
    actor: Entity,
    schedule: &ScheduledAction,
    executed_at: f32,
    busy_until: f32,
) {
    // 行动实体与行动者**都可能已经没了**：同一帧里单位可能先被打死
    // （死亡系统会销毁它），而行动实体的收尾还排在后面。
    // 往不存在的实体上写命令会让 Bevy 直接 panic，所以这里全程走 `get_entity`。
    if let Ok(mut action_commands) = commands.get_entity(action) {
        action_commands.remove::<Committed>().despawn();
    }
    let mut recovery = schedule.recovery_window(executed_at);
    recovery.ready_at = recovery.ready_at.max(busy_until);
    insert_on_actor(commands, actor, recovery);
}

/// 往行动者身上挂组件——**行动者死了就跳过**（同帧被击杀是最常见的情况）。
///
/// 执行器的收尾都排在伤害/死亡之后，所以"我还活着吗"必须每次当场确认，
/// 不能靠调度顺序保证。
pub fn insert_on_actor(commands: &mut Commands, actor: Entity, bundle: impl Bundle) {
    if let Ok(mut actor_commands) = commands.get_entity(actor) {
        actor_commands.insert(bundle);
    }
}

/// 打断：本帧只要出现**新意图**，就把玩家那条**可取消的**未结算行动撤掉。
///
/// 无回合模型里"随时可以改主意"就靠它。判定是纯谓词，四条同时满足才触发：
///
/// 1. 这一帧有新意图（多种意图同帧时取**最后一条**，与各声明系统一致）；
/// 2. 是**玩家**的意图（AI 不走输入消息，自然不参与）；
/// 3. 玩家有一条未结算的行动（`Declared` / `Pending`，还没 `Committed`）；
/// 4. 那条行动**允许取消**（没有 `Uncancellable`；跳跃就是不给撤的）。
///
/// 触发后是一次**原子转移**：撤旧（销毁 + 退 `ActionCost` + 扣 `CancelCost`）→
/// 后面的声明系统照旧接手新意图。它自己不进时间线、没有实体，所以不存在
/// "打断被打断"这种事——能被取消的只有被它撤掉的那个旧行动。
///
/// 实现上就是"把右键那条撤销路径同步触发一次"（写 [`UndoCommand`]）。
#[allow(clippy::too_many_arguments)]
pub fn interrupt_system(
    mut moves: MessageReader<crate::movement::MoveCommand>,
    mut moves_to: MessageReader<crate::movement::MoveToCommand>,
    mut jumps: MessageReader<crate::movement::JumpCommand>,
    mut uses: MessageReader<crate::combat::skills::UseSelectedSkill>,
    mut rolls: MessageReader<crate::combat::defense::RollCommand>,
    mut parries: MessageReader<crate::combat::defense::ParryCommand>,
    mut undo: MessageWriter<UndoCommand>,
) {
    // ⚠️ **只读玩家直接产生的输入层消息**：`FireCommand` / `MeleeCommand` 是
    // `UseSelectedSkill`（或热键）派生的下游消息，晚一帧才出现——监听它们会把
    // "刚刚由自己的意图声明出来的行动"当成新意图撤掉（火球永远发不出去）。
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

/// 撤销系统要看的行动（含调度、花费、取消代价与"能不能撤"）。
///
/// 抽成别名纯粹是因为元组太长（clippy `type_complexity`）。
type UndoableActionQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ScheduledAction,
        Option<&'static ActionCost>,
        Option<&'static CancelCost>,
        Has<Uncancellable>,
        Has<Committed>,
    ),
>;

/// 撤销：把玩家那条**尚未结算**的行动撤掉——销毁行动实体、恢复 `Ready`、
/// 清掉草案记录，并广播 [`ActionCancelled`] 让资源所属的领域退还花费。
///
/// 「尚未结算」= `Declared`（草案）或 `Pending`（已提交、还没到点）；
/// `Committed` 是本帧就要执行的，撤不掉。没有可撤的行动时什么也不做
/// （右键空放不报错，也不提示）。
///
/// 只撤**玩家**的行动：AI 的行动也挂在同一条时间线上，但"谁能按键撤销"
/// 只有 PC（和"谁能按键决策"是同一条约定）。
pub fn undo_system(
    mut commands: Commands,
    mut requests: MessageReader<UndoCommand>,
    mut timeline: ResMut<Timeline>,
    factions: Query<&Faction>,
    actions: UndoableActionQuery<'_, '_>,
    mut cancelled: MessageWriter<ActionCancelled>,
) {
    if requests.read().last().is_none() {
        return;
    }
    for (entity, schedule, cost, penalty, uncancellable, committed) in &actions {
        if committed {
            continue; // 本帧就要执行，来不及撤
        }
        // 行动自己说不给撤（跳跃那种）：连右键也撤不掉
        if uncancellable {
            continue;
        }
        if factions.get(schedule.actor).ok() != Some(&Faction::Player) {
            continue;
        }
        commands.entity(entity).despawn();
        commands.entity(schedule.actor).insert(Ready);
        cancelled.write(ActionCancelled {
            actor: schedule.actor,
            refund: cost.map(|cost| cost.0).unwrap_or_default(),
            // 取消代价缺省为 0（没挂 `CancelCost` 就是免费——"赶路调整"那类）
            penalty: penalty.map(|cost| cost.0).unwrap_or_default(),
        });
        timeline.set_draft(None);
        break; // 一次决策只有一条行动
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::timing;

    /// 回归：**行动者在同一帧先死了**，收尾不能 panic。
    ///
    /// 真实触发路径：单位的移动行动到点要挂后摇时，它已经被这一帧的伤害打死了
    /// （AI 活过来之后很常见）。以前这里直接 `commands.entity(actor).insert(..)`，
    /// 命令应用阶段会以 "Entity despawned" 崩掉整个进程。
    #[test]
    fn ending_an_action_for_a_dead_actor_is_safe() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);

        let actor = app.world_mut().spawn_empty().id();
        let schedule = ScheduledAction::declared_at(actor, timing::MOVE, 0.0);
        let action = app.world_mut().spawn((Committed, schedule)).id();
        app.world_mut().entity_mut(actor).despawn();

        let mut commands = app.world_mut().commands();
        end_action(&mut commands, action, actor, &schedule, 1.0);
        // 命令在这里真正落地：没守住的话这一步就 panic
        app.world_mut().flush();
    }
}
