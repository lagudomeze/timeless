//! 碰撞检测（纯物理）：距离 ≤ 半径之和即命中。

use bevy::prelude::*;

use crate::combat::attributes::HitRadius;
use crate::combat::components::{Collidable, Faction};
use crate::combat::lifecycle::Projectile;

use super::components::CollisionTarget;

/// 射弹碰撞检测。
///
/// 每帧先清掉上一帧的临时标记，再为每个未结束射弹找最近的碰撞目标。
/// 本系统不读伤害 / 护甲，因此新增攻击方式（AOE、追踪弹）只需另写目标获取系统。
pub fn detect_collisions_system(
    mut commands: Commands,
    stale: Query<Entity, With<CollisionTarget>>,
    projectiles: Query<(
        Entity,
        &Transform,
        &HitRadius,
        &Projectile,
        Option<&Faction>,
    )>,
    targets: Query<(Entity, &Transform, &HitRadius, Option<&Faction>), With<Collidable>>,
) {
    for entity in &stale {
        commands.entity(entity).remove::<CollisionTarget>();
    }

    for (projectile, projectile_pos, projectile_radius, state, projectile_faction) in &projectiles {
        if state.finished {
            continue;
        }
        let hit = targets
            .iter()
            .filter(|(target, _, _, target_faction)| {
                *target != projectile
                    && !(projectile_faction.is_some() && *target_faction == projectile_faction)
            })
            .map(|(target, target_pos, target_radius, _)| {
                (
                    target,
                    target_pos.translation.distance(projectile_pos.translation),
                    target_radius.0,
                )
            })
            .filter(|&(_, distance, target_radius)| distance <= projectile_radius.0 + target_radius)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((target, _, _)) = hit {
            commands.entity(projectile).insert(CollisionTarget(target));
        }
    }
}
