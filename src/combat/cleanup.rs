//! 清理层：销毁已结束（`finished`）的攻击实体

use bevy::prelude::*;

use super::components::Projectile;

/// 销毁所有 `finished` 射弹（包括穿透无限但已有命中计数的兜底情况）
pub fn cleanup_finished_attacks_system(mut commands: Commands, q: Query<(Entity, &Projectile)>) {
    for (entity, projectile) in &q {
        if projectile.finished || (projectile.current_hits > 0 && projectile.max_hits == 0) {
            commands.entity(entity).despawn();
        }
    }
}
