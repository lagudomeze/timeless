//! 攻击命中处理：消费 `CollisionTarget`（射弹计数 / 一次性攻击通用）

use bevy::prelude::*;

use super::components::{CollisionTarget, Lifetime, Projectile, Velocity};

/// 命中处理：每消费一个 `CollisionTarget`——
/// - 射弹：`current_hits` +1，达到 `max_hits` 后置 `finished` 并归零速度；
/// - 无 `Projectile` 的一次性攻击（近战横扫等）：仅移除标记。
///
/// 未结束的射弹下一帧可继续飞向新目标。
pub fn manage_projectile_hits_system(
    mut commands: Commands,
    mut q: Query<(
        Entity,
        Option<&mut Projectile>,
        Option<&mut Velocity>,
        &CollisionTarget,
    )>,
) {
    for (entity, projectile, velocity, _target) in &mut q {
        if let Some(mut projectile) = projectile {
            projectile.current_hits += 1;
            if projectile.max_hits > 0 && projectile.current_hits >= projectile.max_hits {
                projectile.finished = true;
                if let Some(mut velocity) = velocity {
                    velocity.0 = Vec3::ZERO;
                }
                info!(
                    "💥 {:?} 命中完毕（{}/{}），射弹结束",
                    entity, projectile.current_hits, projectile.max_hits
                );
            }
        }
        commands.entity(entity).remove::<CollisionTarget>();
    }
}

/// 存活期到期销毁：消耗 `Lifetime` 计时器的一次性攻击实体（近战横扫等）。
pub fn expire_attack_entities_system(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Lifetime)>,
) {
    for (entity, mut lifetime) in &mut q {
        if lifetime.0.tick(time.delta()).is_finished() {
            commands.entity(entity).despawn();
        }
    }
}
