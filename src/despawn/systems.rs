//! 销毁系统：消费 `DeathEvent`，把对应实体 despawn

use bevy::prelude::*;

use crate::events::DeathEvent;

/// 对每条 `DeathEvent` 销毁对应实体（已不存在则跳过）。
pub fn despawn_dead_system(
    mut commands: Commands,
    mut deaths: MessageReader<DeathEvent>,
    alive: Query<()>,
) {
    for death in deaths.read() {
        if alive.get(death.entity).is_ok() {
            info!("🗑 销毁死亡实体 {:?}", death.entity);
            commands.entity(death.entity).despawn();
        }
    }
}
