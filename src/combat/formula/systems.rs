//! 物理伤害计算。

use bevy::prelude::*;

use crate::combat::attributes::{Armor, PhysicalDamage};
use crate::combat::lifecycle::Projectile;
use crate::combat::targeting::CollisionTarget;

use super::events::DamageEvent;
use super::types::DamageType;

/// 物理伤害公式：原始伤害扣护甲，最低为 0。
///
/// 纯函数（零 ECS 依赖），公式可以单独单测；新增减免机制时在这里扩展。
pub fn physical_damage(raw: f32, armor: f32) -> f32 {
    (raw - armor).max(0.0)
}

/// 物理伤害系统：对本帧带 [`CollisionTarget`] 的未结束攻击实体结算并广播 [`DamageEvent`]。
///
/// 只读「自己的伤害组件 + 目标护甲」，不关心目标是碰撞来的还是 AOE 扫到的。
pub fn apply_physical_damage_system(
    mut damage_events: MessageWriter<DamageEvent>,
    attacks: Query<(Option<&Projectile>, &PhysicalDamage, &CollisionTarget)>,
    armor_q: Query<&Armor>,
) {
    for (projectile, physical, target) in &attacks {
        if projectile.is_some_and(|p| p.finished) {
            continue;
        }
        let armor = armor_q.get(target.0).map(|armor| armor.0).unwrap_or(0.0);
        let amount = physical_damage(physical.0, armor);
        if amount > 0.0 {
            damage_events.write(DamageEvent {
                target: target.0,
                amount,
                kind: DamageType::Physical,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn armour_reduces_physical_damage_but_never_below_zero() {
        assert_eq!(physical_damage(10.0, 0.0), 10.0);
        assert_eq!(physical_damage(10.0, 3.0), 7.0);
        assert_eq!(physical_damage(10.0, 30.0), 0.0, "护甲超过伤害时不产生负数");
    }
}
