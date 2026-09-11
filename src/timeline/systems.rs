//! 时间线系统：玩家输入门控、提交桥、调度、后摇恢复。
//!
//! 无回合模型下没有全局阶段，节奏由每个动作自己的前摇 + 后摇决定；
//! 这里只负责四件事：**什么时候停表**、**草案何时升为待执行**、
//! **行动何时到点**、**后摇何时结束**。

use bevy::prelude::*;

use crate::combat::Faction;
use crate::combat::defense::{STAMINA_REGEN_PER_DECISION, Stamina};

use super::components::{BusyRecovery, Committed, Declared, Pending, Ready, ScheduledAction};
use super::events::ActionsCommitted;
use super::resources::{Timeline, TimelineConfig};

/// **唯一的暂停点**：玩家就绪、还在地上、且还没做完决定时冻结虚拟时间。
///
/// 各领域因此不需要任何 `if paused` 分支——移动、后摇、投射物生命周期自动停表。
/// `require_commit = true` 时，**草案存在也仍然冻结**：玩家在「声明 → 确认」之间
/// 需要稳定的战场快照；提交后 `Ready` 被摘掉，时间自然恢复流动去执行它。
///
/// **空中不冻结**：跳跃是不可中断的弹道；若在落地前因为「玩家又就绪了」而停表，
/// 单位会僵在半空。等它落地再等输入。
pub fn timeline_gate_system(
    _config: Res<TimelineConfig>,
    mut timeline: ResMut<Timeline>,
    mut time: ResMut<Time<Virtual>>,
    players: Query<(Entity, &Faction), With<Ready>>,
    factions: Query<&Faction>,
    airborne: Query<(), With<crate::movement::Jumping>>,
) {
    let player_ready = players
        .iter()
        .any(|(_, faction)| *faction == Faction::Player);
    // 没有玩家实体（单测 / 组装之前）一律当作「不等输入」，避免把世界冻住
    let has_player = factions.iter().any(|faction| *faction == Faction::Player);
    let someone_airborne = !airborne.is_empty();

    // `require_commit` 下草案存在也仍然冻结：玩家在「声明 → 确认」之间需要
    // 稳定的战场快照；提交后 `Ready` 被摘掉，时间自然恢复流动去执行它。
    let waiting = has_player && player_ready && !someone_airborne;
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

/// 提交桥：把声明升为待执行。
///
/// 所有载荷声明出来都是 [`Declared`] 状态，因此这里**两种共用一条升格路径**：
///
/// - `require_commit = false`（默认）：声明当帧就升格，输入因此「按下即生效」；
/// - `require_commit = true`：等 [`ActionsCommitted`] 消息（`Enter`）才升格，
///   期间虚拟时间冻结在等玩家确认。
pub fn commit_bridge_system(
    mut commands: Commands,
    config: Res<TimelineConfig>,
    mut timeline: ResMut<Timeline>,
    mut requests: MessageReader<ActionsCommitted>,
    declared: Query<Entity, With<Declared>>,
) {
    if config.require_commit && requests.read().next().is_none() {
        return; // 等玩家按 Enter
    }

    for entity in &declared {
        commands.entity(entity).remove::<Declared>().insert(Pending);
    }
    timeline.set_draft(None);
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
    commands.entity(action).remove::<Committed>().despawn();
    commands
        .entity(actor)
        .insert(schedule.recovery_window(executed_at));
}
