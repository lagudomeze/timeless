//! 时间线系统：暂停门控、提交、调度、窗口收尾。

use bevy::prelude::*;

use super::components::{Committed, Declared, Pending, ScheduledAction};
use super::events::{ActionsCommitted, RoundEnded};
use super::resources::Timeline;

/// 时间线上还没清掉的行动实体（草案 / 待执行 / 已到点未执行）。
type AnyAction<'w, 's> =
    Query<'w, 's, Entity, Or<(With<Declared>, With<Pending>, With<Committed>)>>;

/// 暂停门控：规划阶段冻结虚拟时间，推进阶段恢复。
///
/// 这是「暂停等待用户输入」的唯一实现点——冻结的是时间本身，各领域不需要
/// 任何 `if paused` 分支，移动、计时器、生命周期自动停表。
pub fn pause_during_planning_system(timeline: Res<Timeline>, mut time: ResMut<Time<Virtual>>) {
    if timeline.is_planning() {
        if !time.is_paused() {
            time.pause();
            info!("⏸ 规划阶段：虚拟时间冻结，等待提交（WASD/Space/E 声明，Enter 提交）");
        }
    } else if time.is_paused() {
        time.unpause();
    }
}

/// 提交：把本轮所有草案（`Declared`）定为 `Pending`，并按提交时刻算执行时刻。
///
/// 玩家与 AI 的声明一起提交——这正是 We-Go 的「同时规划、一起结算」。
pub fn commit_actions_system(
    mut commands: Commands,
    mut requests: MessageReader<ActionsCommitted>,
    mut timeline: ResMut<Timeline>,
    now: Res<Time>,
    actions: Query<(Entity, &ScheduledAction), With<Declared>>,
) {
    if requests.read().next().is_none() {
        return;
    }
    let now = now.elapsed_secs();
    let Some(round) = timeline.begin_resolution() else {
        return; // 推进中重复提交：忽略
    };
    let mut committed = 0usize;
    for (entity, action) in &actions {
        commands
            .entity(entity)
            .remove::<Declared>()
            .insert(Pending)
            .insert(action.committed_at(now));
        committed += 1;
    }
    info!("▶ 第 {round} 轮提交：{committed} 条行动进入时间线");
}

/// 调度：推进阶段里到点的行动标记为 `Committed`，交给执行器。
///
/// 调度器只读 [`ScheduledAction`]，不认识任何载荷。
pub fn scheduler_system(
    mut commands: Commands,
    now: Res<Time>,
    actions: Query<(Entity, &ScheduledAction), With<Pending>>,
) {
    let now = now.elapsed_secs();
    for (entity, action) in &actions {
        if now >= action.execute_at {
            debug!("⏱ 行动 {entity:?}（actor {:?}）到点", action.actor);
            commands
                .entity(entity)
                .remove::<Pending>()
                .insert(Committed);
        }
    }
}

/// 窗口收尾：留下没执行完的行动一律作废，回到规划阶段并广播 [`RoundEnded`]。
pub fn end_round_system(
    mut commands: Commands,
    time: Res<Time>,
    mut timeline: ResMut<Timeline>,
    mut ended: MessageWriter<RoundEnded>,
    leftover: AnyAction<'_, '_>,
) {
    if !timeline.tick_resolution(time.delta()) {
        return;
    }
    for entity in &leftover {
        commands.entity(entity).despawn();
    }
    let round = timeline.end_round();
    ended.write(RoundEnded { round });
    info!("⏹ 第 {round} 轮结束：世界重新冻结，等待下一轮提交");
}

/// 清掉某个单位本轮已声明的草案。
///
/// 载荷领域在声明新动作前调用它（We-Go：一个单位同一时刻至多一个行动）；
/// 只读调度组件，因此新载荷不需要改这里。
pub fn clear_declared_actions(
    commands: &mut Commands,
    declared: &Query<(Entity, &ScheduledAction), With<Declared>>,
    actor: Entity,
) {
    for (entity, action) in declared {
        if action.actor == actor {
            commands.entity(entity).despawn();
        }
    }
}
