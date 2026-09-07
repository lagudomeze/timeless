//! 物理伤害：`PhysicalDamage − Armor`，发 `DamageEvent`

use bevy::prelude::*;

use crate::events::DamageEvent;

use super::super::components::{Armor, CollisionTarget, PhysicalDamage, Projectile};

/// 物理伤害系统：对本帧带 `CollisionTarget` 的未结束攻击实体计算
/// `max(PhysicalDamage − Armor, 0)` 并发出 `DamageEvent`。
/// 新增元素伤害（火 / 毒）只需新增系统，本系统不改。
pub fn apply_physical_damage_system(
    mut damage_events: MessageWriter<DamageEvent>,
    attacks: Query<(&Projectile, &PhysicalDamage, &CollisionTarget)>,
    armor_q: Query<&Armor>,
) {
    for (projectile, physical, target) in &attacks {
        if projectile.finished {
            continue;
        }
        let armor = armor_q.get(target.0).map(|a| a.0).unwrap_or(0.0);
        let amount = (physical.0 - armor).max(0.0);
        if amount > 0.0 {
            damage_events.write(DamageEvent {
                target: target.0,
                amount,
            });
        }
    }
}
