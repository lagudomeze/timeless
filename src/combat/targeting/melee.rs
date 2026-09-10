//! 近战形状检测：扇形范围内最近的敌对单位。

use bevy::prelude::*;

use crate::combat::components::{Collidable, Faction};
use crate::combat::lifecycle::HitOnce;

use super::components::{CollisionTarget, MeleeShape};

/// 对每个还没打中的近战攻击，在正前方扇形内找最近的敌对单位挂 [`CollisionTarget`]。
///
/// 命中后置 `HitOnce::spent`，保证一次横扫只结算一次。
pub fn detect_melee_system(
    mut commands: Commands,
    mut attacks: Query<(Entity, &Transform, &Faction, &MeleeShape, &mut HitOnce)>,
    units: Query<(Entity, &Transform, &Faction), With<Collidable>>,
) {
    for (attack_entity, transform, faction, shape, mut hit_once) in &mut attacks {
        if hit_once.spent {
            continue;
        }
        let forward = (transform.rotation * Vec3::Z).normalize_or_zero();
        let best = units
            .iter()
            .filter(|(_, _, unit_faction)| **unit_faction != *faction)
            .map(|(unit, unit_transform, _)| {
                (unit, unit_transform.translation - transform.translation)
            })
            .filter(|(_, to_unit)| {
                let distance = to_unit.length();
                if distance > shape.range {
                    return false;
                }
                distance <= 1e-4 || (*to_unit / distance).dot(forward) >= shape.half_arc.cos()
            })
            .min_by(|(_, a), (_, b)| a.length_squared().total_cmp(&b.length_squared()));
        if let Some((target, _)) = best {
            commands
                .entity(attack_entity)
                .insert(CollisionTarget(target));
            hit_once.spent = true;
        }
    }
}
