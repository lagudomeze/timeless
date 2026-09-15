//! 物理伤害：护甲公式（纯函数）+ 命中系统。
//!
//! 「加一种伤害类型」的全部工作就在这个文件的形状里：
//!
//! ```text
//! 加一个组件（FireDamage…）+ 加一个同形的系统（把「打到了谁」翻译成 DamageEvent）
//! + 在场景工厂里挂上
//! ```
//!
//! 生命值、死亡、日志、撤销都不需要知道新类型存在。

use bevy::prelude::*;

use crate::combat::attributes::{Armor, InterruptPower, PhysicalDamage};
use crate::combat::defense::{DefenseOutcome, Dodging, Parrying};
use crate::combat::lifecycle::{HitOnce, Projectile};
use crate::combat::targeting::CollisionTarget;
use crate::movement::Velocity;
use crate::timeline::InterruptEvent;

use super::domain::{DefenseState, counter_damage, resolve_defense};
use super::events::DamageEvent;

/// 物理伤害公式：原始伤害扣护甲，最低为 0。
///
/// 纯函数（零 Bevy 依赖），公式可以单独单测；新增减免机制时在这里扩展。
pub fn physical_damage(raw: i32, armor: i32) -> i32 {
    (raw - armor).max(0)
}

/// 命中结算（物理）：把「打到了谁」翻译成 [`DamageEvent`]，并处理收尾。
///
/// 这是物理伤害类型的**唯一**系统，一次做四件事：
///
/// 1. **防御判定**：目标在无敌帧里 → 闪开；正招架这一击 → 免伤并反制一半；
/// 2. **伤害落地**：护甲减伤后广播 `DamageEvent`（唯一扣血点在下游）；
/// 3. **打断**：命中时用攻击自带的 [`InterruptPower`] 触发
///    [`InterruptEvent`]（对抗由时间线的 Observer 完成）；
/// 4. **命中收尾**：射弹穿透计数、一次性攻击置位、清掉临时的 `CollisionTarget`。
///
/// 目标获取（谁被打到了）与伤害（打多少）因此仍然解耦：换一种攻击方式只要挂上
/// `PhysicalDamage` + `CollisionTarget`，本系统一行不改。
#[allow(clippy::too_many_arguments)]
pub fn apply_physical_hits_system(
    mut commands: Commands,
    mut damage_events: MessageWriter<DamageEvent>,
    attacks: Query<(
        Entity,
        &PhysicalDamage,
        &CollisionTarget,
        Option<&InterruptPower>,
    )>,
    armors: Query<&Armor>,
    dodging: Query<(), With<Dodging>>,
    parrying: Query<&Parrying>,
    mut hit_once: Query<&mut HitOnce>,
    mut projectiles: Query<(&mut Projectile, Option<&mut Velocity>)>,
) {
    for (attack, damage, marker, power) in &attacks {
        // 已经结束的射弹不再结算（它正等着被清理）
        if projectiles
            .get(attack)
            .is_ok_and(|(projectile, _)| projectile.finished)
        {
            continue;
        }
        let target = marker.0;

        let parry_target = parrying
            .get(target)
            .ok()
            .map(|parry| parry.target_attack.to_bits());
        let outcome = resolve_defense(
            DefenseState {
                dodging: dodging.get(target).is_ok(),
                parrying: parry_target.is_some(),
            },
            attack.to_bits(),
            parry_target,
        );

        let armor = armors.get(target).map(|armor| armor.0).unwrap_or_default();
        let amount = if outcome == DefenseOutcome::Landed {
            physical_damage(damage.0, armor)
        } else {
            0
        };
        if amount > 0 {
            damage_events.write(DamageEvent {
                source: Some(attack),
                target,
                amount,
            });
        }
        // 招架反制：回敬攻击实体一半伤害（向上取整，至少 1 点）
        if outcome == DefenseOutcome::Parried {
            damage_events.write(DamageEvent {
                source: Some(target),
                target: attack,
                amount: counter_damage(damage.0),
            });
        }
        // 打断：打中了才有对抗可言（被闪开 / 被招架 = 没吃到冲击）
        if outcome == DefenseOutcome::Landed
            && let Some(power) = power
            && power.0 != 0
        {
            commands.trigger(InterruptEvent {
                entity: target,
                source: attack,
                power: power.0,
            });
        }

        // 命中计数（射弹穿透 / 一次性攻击）
        if let Ok((mut projectile, velocity)) = projectiles.get_mut(attack) {
            projectile.current_hits += 1;
            if projectile.max_hits > 0 && projectile.current_hits >= projectile.max_hits {
                projectile.finished = true;
                if let Some(mut velocity) = velocity {
                    velocity.0 = Vec3::ZERO;
                }
            }
        }
        if let Ok(mut spent) = hit_once.get_mut(attack) {
            spent.spent = true;
        }

        // 临时标记用完就清
        commands.entity(attack).remove::<CollisionTarget>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn armour_reduces_physical_damage_but_never_below_zero() {
        assert_eq!(physical_damage(10, 0), 10);
        assert_eq!(physical_damage(10, 3), 7);
        assert_eq!(physical_damage(10, 30), 0, "护甲超过伤害时不产生负数");
    }
}
