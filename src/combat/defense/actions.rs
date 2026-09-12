//! 防御域的声明 / 执行系统与过期清理。
//!
//! - 翻滚载荷的工厂在 [`crate::movement`]（退一格与移动是同一个原语），
//!   本模块只负责**声明**与**落地**；
//! - 招架只绑实体，不需要位移，因此载荷与执行器都住在这里；
//! - 过期清理统一在这里，两个标记的语义放在一起看。

use bevy::prelude::*;

use crate::combat::Faction;
use crate::movement::{Cell, Velocity, ground_direction, step_from_axis};
use crate::timeline::{Declared, Ready, Timeline, begin_action};

use super::components::{Dodging, ParryAction, Parrying};
use super::events::{ParryCommand, RollCommand};
use super::stamina::Stamina;

/// 翻滚消耗的精力。
pub const ROLL_COST: u32 = 1;
/// 招架消耗的精力。
pub const PARRY_COST: u32 = 1;
/// 翻滚的无敌帧时长（虚拟秒）。
pub const DODGE_SECS: f32 = 0.5;
/// 翻滚标记的兜底存活时长（虚拟秒）：目标攻击被销毁后不等它自然过期。
pub const PARRY_SECS: f32 = 0.5;
/// 翻滚的位移速度（世界单位 / 秒）：比走路快，但仍然是「退一格」。
pub const ROLL_SPEED: f32 = 8.0;

/// 最近的敌对单位位置（翻滚方向 = 远离它）。
fn nearest_enemy(
    units: &Query<(&Transform, &Faction)>,
    origin: Vec3,
    faction: Faction,
) -> Option<Vec3> {
    units
        .iter()
        .filter(|(_, other)| **other != faction)
        .map(|(transform, _)| transform.translation)
        .min_by(|a, b| {
            a.distance_squared(origin)
                .total_cmp(&b.distance_squared(origin))
        })
}

/// 声明翻滚：`RollCommand` → 远离最近敌对单位退一格 + 无敌帧。
///
/// **玩家与 AI 共用同一条路径**：玩家由键盘写消息、敌人由 `enemy_declare_system`
/// 写同一条消息，都落在这里。因此防御逻辑只有一份实现。
pub fn declare_roll_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    mut timeline: ResMut<Timeline>,
    mut requests: MessageReader<RollCommand>,
    mut rollers: Query<(Entity, &Cell, &Stamina, &Transform, &Faction), With<Ready>>,
    units: Query<(&Transform, &Faction)>,
) {
    if requests.read().last().is_none() {
        return;
    }
    for (entity, cell, stamina, transform, faction) in &mut rollers {
        if !stamina.can_afford(ROLL_COST) {
            continue;
        }

        let origin = transform.translation;
        let faction = *faction;
        let axis = nearest_enemy(&units, origin, faction)
            .map(|threat| {
                let away = origin - threat;
                Vec2::new(away.x, away.z).normalize_or_zero()
            })
            .unwrap_or(Vec2::ZERO);
        let (dx, dz) = step_from_axis(axis);
        let from_cell = *cell;
        let to_cell = Cell::new(cell.x + dx, cell.z + dz);

        let draft = commands
            .spawn_scene(crate::movement::roll_action_scene(
                entity,
                from_cell,
                to_cell,
                now.elapsed_secs(),
            ))
            .id();
        begin_action(&mut commands, &mut timeline, entity, draft);
    }
}

/// 执行翻滚：朝目标格设速度 + 请求「到位时挂无敌帧」 + 扣精力。
///
/// **不直接改 `Cell`**：格子由 [`crate::movement::move_entities_system`] 在真正
/// 到达格中心时更新。否则决策层坐标会立刻跳到目标格，而世界坐标还没动
/// （表现为「滚了但人没动」）。
pub fn roll_executor_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    actions: Query<
        (
            Entity,
            &crate::movement::RollAction,
            &crate::timeline::ScheduledAction,
        ),
        With<crate::timeline::Committed>,
    >,
    mut actors: Query<(&mut Velocity, &mut Stamina, &Cell, &Transform)>,
) {
    let now = now.elapsed_secs();
    for (entity, roll, schedule) in &actions {
        if let Ok((mut velocity, mut stamina, cell, transform)) = actors.get_mut(schedule.actor) {
            stamina.try_spend(ROLL_COST);
            let target = roll.to_cell.center();
            velocity.0 = ground_direction(target - transform.translation.xz()) * ROLL_SPEED;
            commands.entity(schedule.actor).insert((
                crate::movement::MoveGoal { cell: roll.to_cell },
                crate::movement::DodgingOnArrival {
                    expires_at: now + DODGE_SECS,
                },
            ));
            let _ = cell;
        }
        crate::timeline::end_action(&mut commands, entity, schedule.actor, schedule, now);
    }
}

/// 声明招架：`ParryCommand` → 挡下当前正在前摇的那次攻击。
///
/// 找不到威胁（敌人没有待执行的攻击）就不消耗精力、不占用这次决策——
/// 招架是反应，不该因为「空气招架」而白白失去一次行动机会。
pub fn declare_parry_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    mut timeline: ResMut<Timeline>,
    mut requests: MessageReader<ParryCommand>,
    mut players: Query<(Entity, &Stamina), With<Ready>>,
    threats: Query<&crate::timeline::ScheduledAction, With<Declared>>,
) {
    if requests.read().last().is_none() {
        return;
    }
    let Ok((player, stamina)) = players.single_mut() else {
        return;
    };
    if !stamina.can_afford(PARRY_COST) {
        info!("招架失败：精力不足");
        return;
    }
    // 威胁 = 任何「还在草案里」的动作实体（玩家与敌人共用一条声明流程）
    let Some(target_attack) = threats.iter().map(|s| s.actor).next() else {
        return; // 没有威胁：这次输入什么也不做
    };

    let draft = commands
        .spawn_scene(crate::combat::defense::parry_action_scene(
            player,
            target_attack,
            now.elapsed_secs(),
        ))
        .id();
    begin_action(&mut commands, &mut timeline, player, draft);
}

/// 执行招架：给行动者挂 [`Parrying`]，绑定被挡的那次攻击。
pub fn parry_executor_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    actions: Query<
        (Entity, &ParryAction, &crate::timeline::ScheduledAction),
        With<crate::timeline::Committed>,
    >,
    mut actors: Query<&mut Stamina>,
) {
    let now = now.elapsed_secs();
    for (entity, parry, schedule) in &actions {
        if let Ok(mut stamina) = actors.get_mut(schedule.actor) {
            stamina.try_spend(PARRY_COST);
            commands.entity(schedule.actor).insert(Parrying {
                target_attack: parry.target_attack,
                expires_at: now + PARRY_SECS,
            });
        }
        crate::timeline::end_action(&mut commands, entity, schedule.actor, schedule, now);
    }
}

/// 过期清理：[`Dodging`] 到点移除；[`Parrying`] 到点或绑定的攻击消失即移除。
pub fn expire_defense_markers_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    attacks: Query<(), With<crate::timeline::ScheduledAction>>,
    dodging: Query<(Entity, &Dodging)>,
    parrying: Query<(Entity, &Parrying)>,
) {
    let now = now.elapsed_secs();
    for (entity, dodging) in &dodging {
        if now >= dodging.expires_at {
            commands.entity(entity).remove::<Dodging>();
        }
    }
    for (entity, parrying) in &parrying {
        // 绑定的攻击实体已经不存在（打空了 / 被销毁）→ 标记失去意义
        let target_gone = attacks.get(parrying.target_attack).is_err();
        if now >= parrying.expires_at || target_gone {
            commands.entity(entity).remove::<Parrying>();
        }
    }
}
