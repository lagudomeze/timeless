//! 射弹生命周期：消费 `CollisionTarget`，推进穿透计数、归零速度

use bevy::prelude::*;

use super::components::{CollisionTarget, Projectile, Velocity};

/// 命中处理：每消费一个 `CollisionTarget` 计数 +1；
/// 达到 `max_hits` 后置 `finished` 并把 `Velocity` 归零（移动系统自动停住）。
/// 无论是否穿透完毕都移除临时标记，未结束的射弹下一帧可继续飞向新目标。
pub fn manage_projectile_hits_system(
    mut commands: Commands,
    mut q: Query<(Entity, &mut Projectile, &mut Velocity, &CollisionTarget)>,
) {
    for (entity, mut projectile, mut velocity, _target) in &mut q {
        projectile.current_hits += 1;
        if projectile.max_hits > 0 && projectile.current_hits >= projectile.max_hits {
            projectile.finished = true;
            velocity.0 = Vec3::ZERO;
            info!(
                "💥 {:?} 命中完毕（{}/{}），射弹结束",
                entity, projectile.current_hits, projectile.max_hits
            );
        }
        commands.entity(entity).remove::<CollisionTarget>();
    }
}
