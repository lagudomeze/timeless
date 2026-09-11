//! 箭矢场景工厂：视觉 + 通用射弹数据。

use bevy::prelude::*;

use crate::combat::attributes::{AttackFrame, HitRadius, Impact, PhysicalDamage};
use crate::combat::components::Faction;
use crate::combat::lifecycle::Projectile;
use crate::movement::Velocity;

/// 普通箭矢：穿透 1，命中一次即结束。
///
/// `position` / `direction` 由生成方现场计算；阵营随箭矢携带，供碰撞过滤
/// （不打自己人）。裁决参数：帧 4（快）、破势 1（低）。
pub fn arrow_scene(position: Vec3, direction: Vec3, faction: Faction) -> impl Scene {
    let direction = direction.normalize_or_zero();
    let rotation = if direction == Vec3::ZERO {
        Quat::IDENTITY
    } else {
        Quat::from_rotation_arc(Vec3::Z, direction)
    };
    bsn! {
        template_value(faction)
        template_value(Velocity(direction * 12.0))
        Projectile { max_hits: 1, current_hits: 0, finished: false }
        template_value(PhysicalDamage(10.0))
        template_value(AttackFrame(4))
        template_value(Impact(1))
        HitRadius(0.2)
        Transform {
            translation: {position},
            rotation: {rotation},
        }
        Mesh3d(asset_value(Cuboid::new(0.1, 0.1, 0.5)))
        MeshMaterial3d<StandardMaterial>(asset_value(StandardMaterial {
            base_color: Color::srgb(0.9, 0.8, 0.45),
            unlit: true,
            ..default()
        }))
    }
}
