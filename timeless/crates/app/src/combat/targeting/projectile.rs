use super::Target;
use crate::combat::components::Projectile;
use bevy::prelude::*; // 假设有速度组件

pub fn detect_collision(
    mut commands: Commands,
    mut query: Query<(Entity, &Transform), (With<Projectile>, Without<Target>)>,
    enemies: Query<(Entity, &Transform), With<Enemy>>,
) {
    for (entity, transform) in query.iter_mut() {
        for (enemy, e_transform) in enemies.iter() {
            if transform.translation.distance(e_transform.translation) < 1.0 {
                commands.entity(entity).insert(Target(enemy));
                break;
            }
        }
    }
}
