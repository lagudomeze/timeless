//! 扣血系统：消费 `DamageEvent` → 修改 `Health` → 归零时发 `DeathEvent`

use bevy::prelude::*;

use crate::events::{DamageEvent, DeathEvent};

use super::Health;

/// 扣血：对每条 `DamageEvent` 扣减目标 `Health`；生命归零时发出 `DeathEvent`。
/// 目标已死或已不存在时跳过，避免重复死亡消息。
pub fn apply_damage_system(
    mut damages: MessageReader<DamageEvent>,
    mut deaths: MessageWriter<DeathEvent>,
    mut health_q: Query<&mut Health>,
) {
    for damage in damages.read() {
        let Ok(mut health) = health_q.get_mut(damage.target) else {
            continue; // 目标已不存在
        };
        if !health.is_alive() {
            continue;
        }
        health.current = (health.current - damage.amount).max(0.0);
        if !health.is_alive() {
            info!("☠ {:?} 生命归零，发出 DeathEvent", damage.target);
            deaths.write(DeathEvent {
                entity: damage.target,
            });
        }
    }
}
