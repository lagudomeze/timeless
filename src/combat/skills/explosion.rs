//! 爆炸结算：投射物到达目标格后，按**真实距离**对半径内的敌对单位造成伤害。
//!
//! 「决策按格、结算按真实距离」在这里最直观：格是瞄准单位，米是伤害判据。
//! 站在同一格边缘和格中心会真的吃到不同结果。

use bevy::prelude::*;

use crate::combat::formula::DamageEvent;
use crate::combat::formula::types::DamageType;
use crate::combat::{Faction, Health};

use super::fireball::ProjectileArrived;

/// 半径内的伤害候选：`(实体, 真实距离)`，按距离升序。
///
/// 纯函数（零 ECS），因此可以脱离 App 单测「谁被炸到」。
pub fn radial_damage_units(
    origin: Vec3,
    radius: f32,
    candidates: impl IntoIterator<Item = (Entity, Vec3)>,
) -> Vec<(Entity, f32)> {
    let mut hits: Vec<(Entity, f32)> = candidates
        .into_iter()
        .map(|(entity, position)| {
            (
                entity,
                Vec2::new(position.x - origin.x, position.z - origin.z).length(),
            )
        })
        .filter(|(_, distance)| *distance <= radius)
        .collect();
    hits.sort_by(|a, b| {
        a.1.total_cmp(&b.1)
            .then_with(|| a.0.index().cmp(&b.0.index()))
    });
    hits
}

/// 爆炸：消费 [`ProjectileArrived`]，对半径内的敌对单位广播 [`DamageEvent`]，并销毁投射物。
///
/// 范围内**没有单位就是打空了**（不产生任何事件，也不留残留实体）。
pub fn explosion_system(
    mut commands: Commands,
    mut arrived: MessageReader<ProjectileArrived>,
    bodies: Query<(Entity, &Transform, &Faction), With<Health>>,
    mut damage_events: MessageWriter<DamageEvent>,
) {
    for blast in arrived.read() {
        let candidates = bodies
            .iter()
            .filter(|(_, _, faction)| **faction != blast.faction)
            .map(|(entity, transform, _)| (entity, transform.translation));
        let hits = radial_damage_units(blast.origin, blast.radius, candidates);

        for (target, distance) in &hits {
            debug!("💥 爆炸命中 {:?}（距离 {distance:.2}）", target);
            damage_events.write(DamageEvent {
                target: *target,
                amount: blast.damage,
                kind: DamageType::Physical,
            });
        }
        if hits.is_empty() {
            info!("💥 火球落在空地上：范围内没有敌人");
        }
        // 投射物已在到达系统里标记 finished，这里负责彻底清掉
        if let Ok(mut entity) = commands.get_entity(blast.projectile) {
            entity.despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radial_selection_uses_world_distance_not_cells() {
        let mut world = World::new();
        let near = world.spawn_empty().id();
        let edge = world.spawn_empty().id();
        let outside = world.spawn_empty().id();
        let origin = Vec3::new(4.0, 0.0, 4.0); // 格 (1,1) 的中心
        let hits = radial_damage_units(
            origin,
            3.0,
            [
                (near, Vec3::new(4.2, 0.0, 4.1)),    // 约 0.22
                (edge, Vec3::new(4.0, 0.0, 7.0)),    // 正好 3.0，应当算命中
                (outside, Vec3::new(4.0, 0.0, 7.1)), // 3.1，出界
            ],
        );
        assert_eq!(
            hits.iter().map(|(entity, _)| *entity).collect::<Vec<_>>(),
            vec![near, edge],
            "按真实距离取半径内的单位，且边界包含在内"
        );
    }

    #[test]
    fn radial_selection_is_empty_when_nobody_is_near() {
        let mut world = World::new();
        let far = world.spawn_empty().id();
        let hits = radial_damage_units(Vec3::ZERO, 1.0, [(far, Vec3::new(5.0, 0.0, 5.0))]);
        assert!(hits.is_empty(), "空地上爆炸不打任何人");
    }
}
