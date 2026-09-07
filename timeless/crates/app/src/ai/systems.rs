//! 敌人 AI：接近 / 停火
use bevy::prelude::*;

use crate::attacks::arrow_scene;
use crate::combat::{Faction, Velocity};

use super::components::{AttackCooldown, EnemyBrain};

/// 敌人决策：每帧对一个带 `EnemyBrain` 的敌人执行一次。
/// 距离 > engage_range → 追踪；≤ engage_range 且冷却结束 → 面向玩家
/// 生成箭矢并把速度归零（站桩出手）；否则维持当前速度。
pub fn enemy_ai_system(
    time: Res<Time>,
    mut commands: Commands,
    mut enemies: Query<(
        &Transform,
        &mut Velocity,
        &mut AttackCooldown,
        &EnemyBrain,
        &Faction,
    )>,
    units: Query<(&Transform, &Faction)>,
) {
    let player = units
        .iter()
        .find(|(_, faction)| **faction == Faction::Player)
        .map(|(tf, _)| tf.translation);
    let Some(player_pos) = player else {
        return; // 玩家不存在：敌人静止
    };

    for (transform, mut velocity, mut cooldown, brain, faction) in &mut enemies {
        if *faction != Faction::Enemy {
            continue;
        }
        let to_player = player_pos - transform.translation;
        let distance = to_player.length();
        if distance <= brain.engage_range {
            if cooldown.0.tick(time.delta()).just_finished() && distance <= brain.attack_range {
                let direction = to_player / distance.max(f32::EPSILON);
                commands.spawn_scene(arrow_scene(
                    transform.translation + direction * 1.2,
                    direction,
                    Faction::Enemy,
                ));
                velocity.0 = Vec3::ZERO;
                continue;
            }
            if distance > brain.attack_range {
                velocity.0 = to_player.normalize_or_zero() * brain.move_speed;
            }
        } else {
            velocity.0 = Vec3::ZERO;
        }
    }
}
