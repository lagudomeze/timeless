//! 攻击生成系统：消费 `FireCommand`，向最近敌人方向生成箭矢
use bevy::prelude::*;

use crate::combat::Faction;
use crate::health::Health;

use super::{arrow_scene, messages::FireCommand};

/// 玩家开火：找到最近敌人，从玩家前方一小段距离生成普通箭矢。
/// 只负责生成；命中、伤害、清理全部交给 combat 流水线。
pub fn player_fire_arrow_system(
    mut fire_requests: MessageReader<FireCommand>,
    mut commands: Commands,
    units: Query<(&Transform, &Health, &Faction)>,
) {
    if fire_requests.read().next().is_none() {
        return;
    }
    let Some((player_pos, _)) = units
        .iter()
        .find(|(_, _, faction)| **faction == Faction::Player)
        .map(|(tf, _, _)| (tf.translation, tf))
    else {
        return;
    };
    let Some(enemy_pos) = units
        .iter()
        .filter(|(_, _, faction)| **faction == Faction::Enemy)
        .map(|(tf, ..)| tf.translation)
        .min_by(|a, b| {
            a.distance_squared(player_pos)
                .total_cmp(&b.distance_squared(player_pos))
        })
    else {
        return;
    };

    let direction = (enemy_pos - player_pos).normalize_or_zero();
    commands.spawn_scene(arrow_scene(
        player_pos + direction * 1.2,
        direction,
        Faction::Player,
    ));
}
