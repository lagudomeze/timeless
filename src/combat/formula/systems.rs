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
use crate::combat::health::DamageEvent;
use crate::combat::lifecycle::{HitOnce, Projectile};
use crate::combat::targeting::CollisionTarget;
use crate::movement::Velocity;
use crate::timeline::{ActionOf, ActionTiming, DecisionSlot, ScheduledAction};

use super::domain::{DefenseState, counter_damage, interrupt_lands, resolve_defense};
use super::events::InterruptEvent;

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
///    [`InterruptEvent`]（对抗由本域的 [`interrupt_observer`] 当场完成）；
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

/// 3d5：三个五面骰之和（3..=15）。打断对抗用它给双方各加一点运气。
fn roll_3d5() -> i32 {
    (0..3).map(|_| rand::random_range(1..=5)).sum()
}

/// 打断 Observer：命中打过来时，对目标那条**还没到点**的行动做一次掷骰对抗。
///
/// 判定本身是纯逻辑（[`interrupt_lands`]）；这里只负责掷骰、取数据、落地。
///
/// 规则细节：
/// - 只打断 `execute_at > now` 的行动——**本帧到点的已经落地**，打不断；
/// - `power == 0` 直接返回（有力度才谈对抗）；
/// - 打断是"抹掉还没发生的事"，因此不退款、不还精力：那一手白费了。
///
/// 落地动的虽然是时间线的数据（销毁行动实体 + 把决策槽清成 `Empty`），
/// 写法与各领域的执行器一致：**谁判定谁收尾**。
pub fn interrupt_observer(
    trigger: On<InterruptEvent>,
    time: Res<Time<Virtual>>,
    actions: Query<(Entity, &ScheduledAction, &ActionTiming, &ActionOf)>,
    mut commands: Commands,
) {
    let power = trigger.power;
    if power == 0 {
        return;
    }
    let target = trigger.entity;
    let source = trigger.source;
    let now = time.elapsed_secs();
    let Some((action, _, timing, _)) = actions
        .iter()
        .find(|(_, schedule, _, action_of)| action_of.actor() == target && schedule.pending(now))
    else {
        return; // 来不及：这一手已经落地，或者本来就没事可打断
    };

    let attack_roll = roll_3d5();
    let defense_roll = roll_3d5();
    if !interrupt_lands(power, timing.interrupt_resist, attack_roll, defense_roll) {
        debug!("⚖ 打断失败：{source:?} 对 {target:?}");
        return;
    }

    info!("⚡ 打断成功：{source:?} 打掉了 {target:?} 的行动");
    commands.entity(action).despawn();
    if let Ok(mut actor) = commands.get_entity(target) {
        actor.insert(DecisionSlot::Idle { intent: None });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::decision::BUSY_SENTINEL;

    /// 打断对抗不关心具体载荷的节奏：自己造一个。
    const TEST_TIMING: ActionTiming = ActionTiming::new(0.2, 0.3, 3);

    /// 打断 Observer 只用「行动实体 + 骨架」这点东西，不需要整机。
    fn interrupt_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_observer(interrupt_observer);
        app
    }

    #[test]
    fn armour_reduces_physical_damage_but_never_below_zero() {
        assert_eq!(physical_damage(10, 0), 10);
        assert_eq!(physical_damage(10, 3), 7);
        assert_eq!(physical_damage(10, 30), 0, "护甲超过伤害时不产生负数");
    }

    /// 打断：掷骰对抗赢了就销毁行动并清空决策槽；`power == 0` 不做对抗。
    #[test]
    fn interrupt_despawns_a_pending_action_and_frees_the_slot() {
        let mut app = interrupt_app();
        let target = app
            .world_mut()
            .spawn(DecisionSlot::Executing {
                until: BUSY_SENTINEL,
            })
            .id();
        let action = app
            .world_mut()
            .spawn((
                ActionOf(target),
                TEST_TIMING,
                ScheduledAction::declared_at(TEST_TIMING, 0.0),
            ))
            .id();
        let source = app.world_mut().spawn_empty().id();

        // 力度 0：连对抗都不做
        app.world_mut().trigger(InterruptEvent {
            entity: target,
            source,
            power: 0,
        });
        app.world_mut().flush(); // Observer 里的命令要落到世界才看得到
        assert!(app.world().get_entity(action).is_ok());

        // 力度 100：3d5 的差值最大 12，必赢
        app.world_mut().trigger(InterruptEvent {
            entity: target,
            source,
            power: 100,
        });
        app.world_mut().flush();
        assert!(
            app.world().get_entity(action).is_err(),
            "被打断的行动应当消失"
        );
        assert_eq!(
            app.world().get::<DecisionSlot>(target).copied(),
            Some(DecisionSlot::Idle { intent: None }),
            "被打断的人应当立刻拿回决策槽"
        );
    }

    /// 本帧到点的行动已经落地，打不断。
    #[test]
    fn an_action_that_came_due_this_frame_survives_an_interrupt() {
        let mut app = interrupt_app();
        let target = app
            .world_mut()
            .spawn(DecisionSlot::Executing {
                until: BUSY_SENTINEL,
            })
            .id();
        // 声明于 -1s 的行动：它在「现在」早就到点了
        let action = app
            .world_mut()
            .spawn((
                ActionOf(target),
                TEST_TIMING,
                ScheduledAction::declared_at(TEST_TIMING, -1.0),
            ))
            .id();
        let source = app.world_mut().spawn_empty().id();

        app.world_mut().trigger(InterruptEvent {
            entity: target,
            source,
            power: 100,
        });
        app.world_mut().flush();

        assert!(
            app.world().get_entity(action).is_ok(),
            "已经到点的行动打不断（它已经出去了）"
        );
    }
}
