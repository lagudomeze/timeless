//! 攻击实体生命周期清理。

use bevy::prelude::*;

use super::components::{Lifetime, Projectile};

/// 清理已结束的射弹（穿透次数用完 / 到达落点）。
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
