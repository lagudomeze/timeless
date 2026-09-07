//! 近战形状检测：扇形内最接近的敌对单位 → `CollisionTarget`
use bevy::prelude::*;

use super::super::components::{Collidable, CollisionTarget, HitOnce, MeleeShape};
use super::super::faction::Faction;

/// 对每个未命中的近战攻击，在其正前方的扇形内找最近的敌对单位并挂
/// `CollisionTarget`，然后置 `HitOnce::spent`（每段横扫只结算一次）。
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
            .map(|(unit, unit_tf, _)| (unit, unit_tf.translation - transform.translation))
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
