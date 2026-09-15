//! 扣血与死亡销毁。

use bevy::prelude::*;

use crate::combat::formula::DamageEvent;

use super::components::Health;
use super::events::DeathEvent;

/// **唯一的扣血点**：把 [`DamageEvent`] 落到 `Health` 上，并在首次归零时发
/// [`DeathEvent`]。
///
/// 两条规则：
///
/// 1. **不提前终止**：扣到负数继续扣（`current -= amount`），因此不存在
///    "最后一下只扣到 0"的账面误差；
/// 2. **只发一次死亡消息**：判据是 `was_alive && now_dead`，同一帧的多段伤害
///    也只会有一条 `DeathEvent`。
pub fn apply_damage_system(
    mut damages: MessageReader<DamageEvent>,
    mut deaths: MessageWriter<DeathEvent>,
    mut healths: Query<&mut Health>,
) {
    for damage in damages.read() {
        let Ok(mut health) = healths.get_mut(damage.target) else {
            continue; // 目标已不存在（同帧被打死过）
        };
        let was_alive = health.is_alive();
        health.current -= damage.amount;
        if was_alive && !health.is_alive() {
            info!("☠ {:?} 生命归零，发出 DeathEvent", damage.target);
            deaths.write(DeathEvent {
                entity: damage.target,
                killer: damage.source,
            });
        }
    }
}

/// 死亡销毁（帧末）：生命值 ≤ 0 的实体从世界移除。
///
/// 直接看 `Health` 而不是消费 [`DeathEvent`]：任何把血扣到 ≤ 0 的路径
/// （伤害、将来的中毒 / 献祭）都被同一条规则收口，不需要各自记得发消息。
pub fn despawn_dead_system(mut commands: Commands, dead: Query<(Entity, &Health)>) {
    for (entity, health) in &dead {
        if health.is_alive() {
            continue;
        }
        info!("🗑 销毁死亡实体 {entity:?}");
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::formula::DamageEvent;

    fn damage_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<DamageEvent>()
            .add_message::<DeathEvent>()
            .add_systems(Update, (apply_damage_system, despawn_dead_system).chain());
        app
    }

    /// 致命伤：扣到负数、只发一条死亡消息、实体在帧末被销毁。
    #[test]
    fn lethal_damage_kills_once_and_cleans_up() {
        let mut app = damage_app();
        let victim = app.world_mut().spawn(Health::new(5)).id();

        app.world_mut().write_message(DamageEvent {
            source: None,
            target: victim,
            amount: 12,
        });
        app.update();
        app.update();

        assert!(
            app.world().get_entity(victim).is_err(),
            "生命归零的实体应当在帧末销毁"
        );
    }

    /// 已经死掉的实体再吃伤害：不再发第二条死亡消息（血继续往负走）。
    #[test]
    fn a_corpse_does_not_report_a_second_death() {
        #[derive(Resource, Default)]
        struct Deaths(usize);
        fn count(mut deaths: MessageReader<DeathEvent>, mut counter: ResMut<Deaths>) {
            counter.0 += deaths.read().count();
        }

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Deaths>()
            .add_message::<DamageEvent>()
            .add_message::<DeathEvent>()
            .add_systems(Update, (apply_damage_system, count).chain());
        let victim = app.world_mut().spawn(Health::new(5)).id();

        for _ in 0..2 {
            app.world_mut().write_message(DamageEvent {
                source: None,
                target: victim,
                amount: 12,
            });
            app.update();
        }

        assert_eq!(app.world().resource::<Deaths>().0, 1, "死亡只报一次");
        assert!(
            app.world().get::<Health>(victim).unwrap().current < 0,
            "伤害不提前终止，血继续往负走"
        );
    }
}
