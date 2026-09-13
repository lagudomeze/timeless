//! 技能行动：载荷 + 工厂 + 声明 / 执行系统。
//!
//! 每个技能 = 一个载荷组件 + 一个行动工厂 + 一个执行器。执行器到点后生成的
//! 攻击实体（箭矢 / 横扫）走通用战斗流水线，时间线完全不参与。
//!
//! ⚠️ **箭矢当前未被玩家输入触发**：玩家的远程手段是火球
//! （[`super::fireball`]，锁格 + AoE）。这里保留箭矢作为**单体碰撞投射物**的
//! 参考实现与测试夹具（`shoot_action_executor_system` / `arrow_scene`），
//! 计划用于将来的「单体狙击」技能；`declare_skill_system` 因此没有注册进插件
//! （避免和 `declare_fireball_system` 抢同一条 `FireCommand`）。

use bevy::prelude::*;

use crate::combat::components::Faction;
use crate::timeline::{
    Committed, Declared, Ready, ScheduledAction, Timeline, begin_action, end_action, timing,
};

use super::arrow::arrow_scene;
use super::events::{FireCommand, MeleeCommand};
use super::melee::melee_scene;

/// 射击载荷：朝最近敌人放一支箭。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ShootAction;

/// 近战载荷：朝最近敌人横扫一次。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MeleeAction;

/// 射击行动工厂。
pub fn shoot_action_scene(actor: Entity, now: f32) -> impl Scene {
    let schedule = ScheduledAction::declared_at(actor, timing::SHOOT, now);
    bsn! {
        ShootAction
        template_value(schedule)
        Declared
    }
}

/// 近战行动工厂。
pub fn melee_action_scene(actor: Entity, now: f32) -> impl Scene {
    let schedule = ScheduledAction::declared_at(actor, timing::MELEE, now);
    bsn! {
        MeleeAction
        template_value(schedule)
        // 抡出去再收招要付一点精力（比火球轻）
        template_value(crate::timeline::CancelCost(1))
        Declared
    }
}

/// 声明技能：`FireCommand` / `MeleeCommand` → 玩家的一条技能行动。
///
/// 只在玩家有 [`Ready`] 时接受；同一帧同时按下两个键时以射击为准。
pub fn declare_skill_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    mut timeline: ResMut<Timeline>,
    mut fires: MessageReader<FireCommand>,
    mut melees: MessageReader<MeleeCommand>,
    mut blocked: MessageWriter<crate::timeline::ActionBlocked>,
    players: Query<(Entity, &Faction), With<Ready>>,
) {
    let fire = fires.read().last().is_some();
    let melee = melees.read().last().is_some();
    if !fire && !melee {
        return;
    }
    let player = players
        .iter()
        .find(|(_, faction)| **faction == Faction::Player)
        .map(|(entity, _)| entity);
    let Some(player) = player else {
        blocked.write(crate::timeline::ActionBlocked::BUSY);
        return; // 忙（前摇 / 后摇中）或没有玩家
    };

    let now = now.elapsed_secs();
    let draft = if fire {
        commands.spawn_scene(shoot_action_scene(player, now)).id()
    } else {
        commands.spawn_scene(melee_action_scene(player, now)).id()
    };
    begin_action(&mut commands, &mut timeline, player, draft);
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
    now: Res<Time<Virtual>>,
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
        end_action(
            &mut commands,
            entity,
            schedule.actor,
            schedule,
            now.elapsed_secs(),
        );
    }
}

/// 执行近战：到点后在行动者前方生成一次性横扫，随后销毁行动实体。
pub fn melee_action_executor_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
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
        end_action(
            &mut commands,
            entity,
            schedule.actor,
            schedule,
            now.elapsed_secs(),
        );
    }
}
