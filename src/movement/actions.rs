//! 移动行动：载荷 + 工厂 + 声明 / 执行系统。
//!
//! 载荷与它的一切都住在移动领域；时间线只负责「什么时候到点」，
//! 因此新增移动方式（冲刺、后退）只需在这里加载荷与执行器。

use bevy::prelude::*;

use crate::combat::Faction;
use crate::timeline::{Committed, Declared, ScheduledAction, Timeline, clear_declared_actions};

use super::components::{MoveSpeed, Velocity};
use super::events::MoveCommand;

/// 移动载荷：朝 `axis`（归一化平面方向）走完一个推进窗口。
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct MoveAction {
    pub axis: Vec2,
}

/// 移动前摇（虚拟秒）：提交后 0.15 秒起步。
const MOVE_WINDUP: f32 = 0.15;

/// 行动实体工厂：载荷 + 调度数据 + 草案状态标记。
pub fn move_action_scene(actor: Entity, axis: Vec2) -> impl Scene {
    let schedule = ScheduledAction::draft(actor, MOVE_WINDUP);
    bsn! {
        MoveAction { axis: {axis} }
        template_value(schedule)
        Declared
    }
}

/// 规划阶段的声明：`MoveCommand` → 玩家的一条移动行动草案。
///
/// 输入每帧都会发消息，所以这里必须区分「玩家又操作了」和「键还按着」：
///
/// - **方向没变、草案还在** → 什么都不做。否则按着 W 会每帧重建草案，
///   把玩家刚声明的技能草案顶掉（后声明覆盖先声明的前提是「真的是新声明」）；
/// - **方向变了** → 一次新的移动声明：覆盖当前草案（含技能草案）；
/// - **松手（零方向）** → 只收回已有的移动草案，技能草案保留；
/// - **没有草案** → 按当前方向补一条，支持按住方向键连续几轮移动。
pub fn declare_move_system(
    mut commands: Commands,
    timeline: Res<Timeline>,
    mut requests: MessageReader<MoveCommand>,
    units: Query<(Entity, &Faction)>,
    declared: Query<(Entity, &ScheduledAction), With<Declared>>,
    drafts: Query<&MoveAction>,
    mut last_axis: Local<Vec2>,
) {
    // 先读消息：推进阶段按下的键一律忽略（不留到下一轮，行为才可预期）
    let Some(axis) = requests.read().last().map(|command| command.axis) else {
        return;
    };
    if !timeline.is_planning() {
        return; // 推进阶段不接受新声明
    }
    let Some(player) = units
        .iter()
        .find(|(_, faction)| **faction == Faction::Player)
        .map(|(entity, _)| entity)
    else {
        return;
    };

    let draft = declared
        .iter()
        .find(|(_, action)| action.actor == player)
        .map(|(entity, _)| entity);

    // 移动输入没变化：不干预当前草案（它可能正是玩家声明好的技能）
    if axis == *last_axis && draft.is_some() {
        return;
    }
    *last_axis = axis;

    if axis == Vec2::ZERO {
        // 松手：只收回移动草案；技能草案不受影响
        if let Some(entity) = draft.filter(|entity| drafts.contains(*entity)) {
            commands.entity(entity).despawn();
        }
        return;
    }

    // 新方向 = 新声明：覆盖当前草案（可能是技能）
    clear_declared_actions(&mut commands, &declared, player);
    commands.spawn_scene(move_action_scene(player, axis));
}

/// 执行：到点的移动行动 → 给行动者设置速度（窗口内匀速前进），随后销毁行动实体。
///
/// 单位在窗口结束时由 [`stop_on_round_end_system`](super::systems::stop_on_round_end_system)
/// 停下，因此这里不需要关心「走多久」。
pub fn move_action_executor_system(
    mut commands: Commands,
    actions: Query<(Entity, &MoveAction, &ScheduledAction), With<Committed>>,
    mut actors: Query<(&MoveSpeed, &mut Velocity)>,
) {
    for (entity, action, schedule) in &actions {
        if let Ok((speed, mut velocity)) = actors.get_mut(schedule.actor) {
            velocity.0 = action.axis.extend(0.0) * speed.0;
        }
        commands.entity(entity).despawn();
    }
}
