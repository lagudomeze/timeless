//! 碰撞检测（纯物理）：射弹与 `Collidable` 实体距离 ≤ 半径和 → 挂 `CollisionTarget`

use bevy::prelude::*;

use super::super::components::{Collidable, CollisionTarget, HitRadius, Projectile};

/// 每帧：
/// 1. 先清掉上一帧残留的 `CollisionTarget`（临时标记只存活一帧的检测周期）；
/// 2. 为每个未结束射弹找出最近的碰撞目标并挂标记。
///
/// 本系统不读取伤害 / 阵营组件，因此新增攻击方式（AOE / 近战形状 / 追踪弹）
/// 只需另写目标获取系统，碰撞与伤害链路零改动。
pub fn detect_collisions_system(
    mut commands: Commands,
    stale: Query<Entity, With<CollisionTarget>>,
    projectiles: Query<(Entity, &Transform, &HitRadius, &Projectile)>,
    targets: Query<(Entity, &Transform, &HitRadius), With<Collidable>>,
) {
    for entity in &stale {
        commands.entity(entity).remove::<CollisionTarget>();
    }

    for (projectile, p_pos, p_radius, state) in &projectiles {
        if state.finished {
            continue;
        }
        let hit = targets
            .iter()
            .filter(|(target, ..)| *target != projectile)
            .map(|(target, t_pos, t_radius)| {
                (
                    target,
                    t_pos.translation.distance(p_pos.translation),
                    t_radius.0,
                )
            })
            .filter(|&(_, distance, t_radius)| distance <= p_radius.0 + t_radius)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((target, _, _)) = hit {
            commands.entity(projectile).insert(CollisionTarget(target));
        }
    }
}
