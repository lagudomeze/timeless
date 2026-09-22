//! 生命值组件。

use bevy::prelude::*;

/// 当前 / 最大生命值（整数：纯减法，可交换，没有浮点边界）。
///
/// **允许扣到负数**：伤害不提前终止，`is_alive` 只回答"还站着吗"。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Health {
    pub current: i32,
    pub max: i32,
}

impl Default for Health {
    fn default() -> Self {
        Self::new(100)
    }
}

impl Health {
    /// 满血单位。
    pub fn new(max: i32) -> Self {
        Self { current: max, max }
    }

    /// 是否还活着。
    pub fn is_alive(&self) -> bool {
        self.current > 0
    }
}

/// 目标生命归零（**同一实体只发一次**：扣血时比较扣前 / 扣后）。
///
/// 写：[`apply_damage_system`](super::systems::apply_damage_system)；
/// 消费：战斗日志（[`crate::presentation::BattleLog`]）。
///
/// 销毁不由它驱动：`despawn_dead_system` 直接看 `Health.current <= 0`，
/// 因此"谁把血扣成负的"都能被清理，不会漏。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeathEvent {
    /// 阵亡的实体
    pub entity: Entity,
    /// 击杀者（伤害来源；环境伤害可以是 `None`）
    pub killer: Option<Entity>,
}

/// 一次**已经算完减免**的伤害（纯减法，可交换）。
///
/// 写：各伤害类型的命中系统（[`apply_physical_hits_system`](super::apply_physical_hits_system)、
/// 爆炸、将来的火焰 / 毒…）；
/// 消费：[`apply_damage_system`](crate::combat::health::apply_damage_system)（唯一的扣血点）、
/// 战斗日志（[`crate::presentation::BattleLog`]）。
///
/// `source` 只为复盘 / 击杀归属存在（死亡消息要写清"谁杀的"），结算本身不看它。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageEvent {
    /// 伤害来源（攻击实体 / 施法者；环境伤害可以是 `None`）
    pub source: Option<Entity>,
    /// 被打的目标
    pub target: Entity,
    /// 扣多少血
    pub amount: i32,
}

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
        //todo maybe do despawn recursively
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

pub mod plugin;
pub use plugin::HealthPlugin;

/// 扣血与死亡在本域系统链里的位置（跨子域的先后由 [`CombatPlugin`](super::CombatPlugin) 编排）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HealthSet;
