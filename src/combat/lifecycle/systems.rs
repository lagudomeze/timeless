//! 命中处理与生命周期清理。

use bevy::prelude::*;

use crate::combat::targeting::CollisionTarget;
use crate::movement::Velocity;

use super::components::{Lifetime, Projectile};

/// 命中处理：消费 [`CollisionTarget`]。
///
/// ⚠️ **当前未注册**：职责已由两阶段结算的 `phase2_apply_system` 接管
/// （命中计数 + 归零速度 + 清标记在同一处落地，避免两个系统抢同一份
/// `Projectile` / `CollisionTarget`）。保留为**穿透 / 多命中投射物**的参考实现，
/// 将来做「穿透箭」时按它的循环计数逻辑扩展。
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
