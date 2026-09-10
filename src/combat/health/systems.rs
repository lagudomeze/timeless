//! 扣血与死亡销毁。

use bevy::prelude::*;

use crate::combat::formula::DamageEvent;

use super::components::Health;
use super::events::{DeathEvent, ModifyHealthEvent};

/// 伤害 → 扣血请求：把 [`DamageEvent`] 翻译成负向 [`ModifyHealthEvent`]。
///
/// 生命值只认自己的消息入口，治疗 / 中毒 / 再生等其它来源也走同一条路；
/// 伤害公式因此完全不认识 `Health`，两边各自演化。
pub fn request_damage_system(
    mut damages: MessageReader<DamageEvent>,
    mut modify_events: MessageWriter<ModifyHealthEvent>,
) {
    for damage in damages.read() {
        modify_events.write(ModifyHealthEvent {
            target: damage.target,
            amount: -damage.amount,
        });
    }
}

/// 扣血 / 治疗：对每条 [`ModifyHealthEvent`] 改目标血量，归零时发 [`DeathEvent`]。
///
/// 目标不存在或已死亡时跳过，避免重复发死亡消息。
pub fn apply_damage(
    mut modify_events: MessageReader<ModifyHealthEvent>,
    mut deaths: MessageWriter<DeathEvent>,
    mut health_q: Query<&mut Health>,
) {
    for event in modify_events.read() {
        let Ok(mut health) = health_q.get_mut(event.target) else {
            continue; // 目标已不存在
        };
        if !health.is_alive() {
            continue;
        }
        health.current = (health.current + event.amount).max(0.0);
        if !health.is_alive() {
            info!("☠ {:?} 生命归零，发出 DeathEvent", event.target);
            deaths.write(DeathEvent {
                entity: event.target,
            });
        }
    }
}

/// 死亡销毁：消费 [`DeathEvent`]，把对应实体从世界移除。
///
/// 消息与消费系统同属生命值子域；实体已不存在时跳过。
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
