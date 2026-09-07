//! 通用移动系统：所有带 `Velocity` 的实体按速度 × dt 位移

use bevy::prelude::*;

use super::components::{Projectile, Velocity};

/// 每帧 位置 += 速度 × dt。射弹已结束（`finished`）或速度为 0 时跳过。
pub fn move_entities_system(
    time: Res<Time>,
    mut q: Query<(&mut Transform, &Velocity, Option<&Projectile>)>,
) {
    let dt = time.delta_secs();
    for (mut transform, velocity, projectile) in &mut q {
        if projectile.is_some_and(|p| p.finished) || velocity.0 == Vec3::ZERO {
            continue;
        }
        transform.translation += velocity.0 * dt;
    }
}
