//! 技能行动：载荷 + 工厂 + 声明 / 执行系统。
//!
//! 每个技能 = 一个载荷组件 + 一个行动工厂 + 一个执行器。执行器到点后生成的
//! 攻击实体（箭矢 / 横扫）走通用战斗流水线，时间线完全不参与。

use bevy::prelude::*;

use crate::combat::components::Faction;
use crate::timeline::{Committed, Declared, ScheduledAction, Timeline, clear_declared_actions};

use super::arrow::arrow_scene;
use super::events::{FireCommand, MeleeCommand};
use super::melee::melee_scene;

/// 射击载荷：朝最近敌人放一支箭。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ShootAction;

/// 近战载荷：朝最近敌人横扫一次。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MeleeAction;

/// 前摇（虚拟秒）。
const SHOOT_WINDUP: f32 = 0.30;
const MELEE_WINDUP: f32 = 0.20;

/// 射击行动工厂。
pub fn shoot_action_scene(actor: Entity) -> impl Scene {
    let schedule = ScheduledAction::draft(actor, SHOOT_WINDUP);
    bsn! {
        ShootAction
        template_value(schedule)
        Declared
    }
}

/// 近战行动工厂。
pub fn melee_action_scene(actor: Entity) -> impl Scene {
    let schedule = ScheduledAction::draft(actor, MELEE_WINDUP);
    bsn! {
        MeleeAction
        template_value(schedule)
        Declared
    }
}

/// 规划阶段的声明：`FireCommand` / `MeleeCommand` → 玩家的一条技能行动草案。
///
/// 同一轮里后声明覆盖先声明；同一帧同时按下两个键时以射击为准。
pub fn declare_skill_system(
    mut commands: Commands,
    timeline: Res<Timeline>,
    mut fires: MessageReader<FireCommand>,
    mut melees: MessageReader<MeleeCommand>,
    units: Query<(Entity, &Faction)>,
    declared: Query<(Entity, &ScheduledAction), With<Declared>>,
) {
    // 先读消息：推进阶段按下的键一律忽略（不留到下一轮，行为才可预期）
    let fire = fires.read().last().is_some();
    let melee = melees.read().last().is_some();
    if !timeline.is_planning() {
        return; // 推进阶段不接受新声明
    }
    if !fire && !melee {
        return;
    }
    let Some(player) = units
        .iter()
        .find(|(_, faction)| **faction == Faction::Player)
        .map(|(entity, _)| entity)
    else {
        return;
    };

    clear_declared_actions(&mut commands, &declared, player);
    if fire {
        commands.spawn_scene(shoot_action_scene(player));
    } else {
        commands.spawn_scene(melee_action_scene(player));
    }
}

/// 敌对目标：离 `origin` 最近的、阵营不同的单位位置。
fn nearest_enemy(
    units: &Query<(&Transform, &Faction)>,
    origin: Vec3,
    faction: Faction,
) -> Option<Vec3> {
    units
        .iter()
        .filter(|(_, unit_faction)| **unit_faction != faction)
        .map(|(transform, _)| transform.translation)
        .min_by(|a, b| {
            a.distance_squared(origin)
                .total_cmp(&b.distance_squared(origin))
        })
}

/// 执行射击：到点后从行动者位置朝最近敌人放箭，随后销毁行动实体。
pub fn shoot_action_executor_system(
    mut commands: Commands,
    actions: Query<(Entity, &ScheduledAction, &ShootAction), With<Committed>>,
    units: Query<(&Transform, &Faction)>,
) {
    for (entity, schedule, _) in &actions {
        if let Ok((transform, faction)) = units.get(schedule.actor) {
            let origin = transform.translation;
            let faction = *faction;
            if let Some(target) = nearest_enemy(&units, origin, faction) {
                let direction = (target - origin).normalize_or_zero();
                commands.spawn_scene(arrow_scene(origin + direction * 1.2, direction, faction));
            }
        }
        commands.entity(entity).despawn();
    }
}

/// 执行近战：到点后在行动者前方生成一次性横扫，随后销毁行动实体。
pub fn melee_action_executor_system(
    mut commands: Commands,
    actions: Query<(Entity, &ScheduledAction, &MeleeAction), With<Committed>>,
    units: Query<(&Transform, &Faction)>,
) {
    for (entity, schedule, _) in &actions {
        if let Ok((transform, faction)) = units.get(schedule.actor) {
            let origin = transform.translation;
            let faction = *faction;
            if let Some(target) = nearest_enemy(&units, origin, faction) {
                let direction = (target - origin).normalize_or_zero();
                commands.spawn_scene(melee_scene(origin + direction * 0.6, direction, faction));
            }
        }
        commands.entity(entity).despawn();
    }
}
