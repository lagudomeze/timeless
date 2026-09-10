//! 命中处理与生命周期清理。

use bevy::prelude::*;

use crate::combat::targeting::CollisionTarget;
use crate::movement::Velocity;

use super::components::{Lifetime, Projectile};

/// 命中处理：消费 [`CollisionTarget`]。
///
/// - 射弹：命中计数 +1，达到上限就置 `finished` 并归零速度（位置推进立刻停）；
/// - 没有 `Projectile` 的一次性攻击（近战横扫）：只移除标记。
///
/// 未结束的射弹下一帧可以继续飞向新目标。
pub fn manage_projectile_hits_system(
    mut commands: Commands,
    mut attacks: Query<(
        Entity,
        Option<&mut Projectile>,
        Option<&mut Velocity>,
        &CollisionTarget,
    )>,
) {
    for (entity, projectile, velocity, _target) in &mut attacks {
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

/// 清理已结束的射弹。
pub fn cleanup_finished_attacks_system(
    mut commands: Commands,
    projectiles: Query<(Entity, &Projectile)>,
) {
    for (entity, projectile) in &projectiles {
        if projectile.finished || (projectile.current_hits > 0 && projectile.max_hits == 0) {
            commands.entity(entity).despawn();
        }
    }
}

/// 存活期到期销毁：一次性攻击实体（近战横扫等）。
pub fn expire_attack_entities_system(
    time: Res<Time>,
    mut commands: Commands,
    mut attacks: Query<(Entity, &mut Lifetime)>,
) {
    for (entity, mut lifetime) in &mut attacks {
        if lifetime.0.tick(time.delta()).is_finished() {
            commands.entity(entity).despawn();
        }
    }
}
