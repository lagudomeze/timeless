//! 敌人 AI：在规划阶段声明本轮的移动 / 射击。
//!
//! AI 与玩家遵守同一套规则：**同时规划、一起结算**。它只声明行动实体，
//! 到点后由移动 / 技能领域的执行器落地，因此 AI 不碰规则、也不碰表现。

use bevy::prelude::*;

use crate::combat::Faction;
use crate::combat::skills::shoot_action_scene;
use crate::movement::move_action_scene;
use crate::timeline::{RoundEnded, ScheduledAction, Timeline};

use super::components::{AttackCooldown, EnemyBrain};

/// 敌人决策：每个敌人在规划阶段声明**至多一条**行动（声明过就跳过）。
///
/// - 距离 > `engage_range`：按兵不动；
/// - 距离 ≤ `attack_range` 且冷却结束：声明射击；
/// - 其余情况：朝玩家声明移动。
pub fn enemy_declare_system(
    mut commands: Commands,
    timeline: Res<Timeline>,
    mut enemies: Query<(Entity, &Transform, &mut AttackCooldown, &EnemyBrain)>,
    units: Query<(&Transform, &Faction)>,
    declared: Query<&ScheduledAction>,
) {
    if !timeline.is_planning() {
        return; // 推进阶段不接受新声明
    }
    let Some(player_pos) = units
        .iter()
        .find(|(_, faction)| **faction == Faction::Player)
        .map(|(transform, _)| transform.translation)
    else {
        return; // 玩家不存在：敌人不动
    };

    for (entity, transform, mut cooldown, brain) in &mut enemies {
        if declared.iter().any(|action| action.actor == entity) {
            continue; // 本轮已经声明过
        }
        let to_player = player_pos - transform.translation;
        let distance = to_player.length();
        if distance > brain.engage_range {
            continue; // 太远：按兵不动
        }
        if distance <= brain.attack_range && cooldown.is_ready() {
            commands.spawn_scene(shoot_action_scene(entity));
            cooldown.after_shot();
            continue;
        }
        let axis = Vec2::new(to_player.x, to_player.z).normalize_or_zero();
        commands.spawn_scene(move_action_scene(entity, axis));
    }
}

/// 每轮结束：冷却走一格。
pub fn tick_attack_cooldown_system(
    mut ended: MessageReader<RoundEnded>,
    mut cooldowns: Query<&mut AttackCooldown>,
) {
    for _ in ended.read() {
        for mut cooldown in &mut cooldowns {
            cooldown.tick_round();
        }
    }
}
