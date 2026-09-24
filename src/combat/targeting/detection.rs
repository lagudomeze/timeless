//! 碰撞检测（纯物理）：距离 ≤ 半径之和即命中。

use bevy::prelude::*;

use crate::combat::attributes::HitRadius;
use crate::combat::components::{Collidable, Faction};
use crate::combat::lifecycle::Projectile;

use super::components::CollisionTarget;

/// 射弹碰撞检测。
///
/// 每帧先清掉上一帧的临时标记，再为每个未结束射弹找最近的碰撞目标。
/// 本系统不读伤害 / 护甲，因此新增攻击方式（AOE、追踪弹）只需另写目标获取系统。
///
/// ⚠️ **判据是地面平面距离（XZ），不是三维距离**：单位站在**地表**上，`y` 随地形
/// 起伏；箭矢从射手脚底平飞，而目标可能站在高一级或低一级的台阶上——
/// 用三维距离的话，**一格之高差就够让这一箭"擦着头皮飞过去"**（实测：完全打不中）。
/// 这与 AoE 的判据（`radial_damage_units` 只取 `xz` 分量）是同一条设计：
/// **决策按格、结算按地面距离**，高度只是"贴地"的表现，不该参与命中。
pub fn detect_collisions_system(
    mut commands: Commands,
    stale: Query<Entity, With<CollisionTarget>>,
    projectiles: Query<(
        Entity,
        &Transform,
        &HitRadius,
        &Projectile,
        Option<&Faction>,
    )>,
    targets: Query<(Entity, &Transform, &HitRadius, Option<&Faction>), With<Collidable>>,
) {
    for entity in &stale {
        commands.entity(entity).remove::<CollisionTarget>();
    }

    for (projectile, projectile_pos, projectile_radius, state, projectile_faction) in &projectiles {
        if state.finished {
            continue;
        }
        let hit = targets
            .iter()
            .filter(|(target, _, _, target_faction)| {
                *target != projectile
                    && !(projectile_faction.is_some() && *target_faction == projectile_faction)
            })
            .map(|(target, target_pos, target_radius, _)| {
                (
                    target,
                    ground_distance(projectile_pos.translation, target_pos.translation),
                    target_radius.0,
                )
            })
            .filter(|&(_, distance, target_radius)| distance <= projectile_radius.0 + target_radius)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((target, _, _)) = hit {
            commands.entity(projectile).insert(CollisionTarget(target));
        }
    }
}

/// 两个位置在地面平面上的距离（忽略高度）。
pub fn ground_distance(from: Vec3, to: Vec3) -> f32 {
    Vec2::new(to.x - from.x, to.z - from.z).length()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 地面距离**丢掉 `y`**：这正是"台阶不该让箭擦着头皮飞过去"那条判据。
    #[test]
    fn ground_distance_ignores_height() {
        let flat = ground_distance(Vec3::new(0.0, 0.0, 0.0), Vec3::new(3.0, 0.0, 4.0));
        assert_eq!(flat, 5.0, "平面距离就是勾股");
        let stepped = ground_distance(Vec3::new(0.0, 0.0, 0.0), Vec3::new(3.0, -7.0, 4.0));
        assert_eq!(
            stepped,
            flat,
            "高一格 / 低一格不该改变命中判据（三维距离会算成 {:.2}）",
            Vec3::new(3.0, -7.0, 4.0).length()
        );
    }
}
