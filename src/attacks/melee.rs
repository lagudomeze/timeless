//! 近战横扫场景工厂：一次性攻击实体 + 扇形命中形状
use bevy::prelude::*;

use crate::combat::{Faction, HitOnce, Lifetime, MeleeShape, PhysicalDamage};

/// 玩家近战横扫：短命攻击实体，命中由 `targeting::detect_melee_system`
/// 负责，伤害走通用流水线，`Lifetime` 到期自动销毁。
pub fn melee_scene(position: Vec3, direction: Vec3, faction: Faction) -> impl Scene {
    let direction = direction.normalize_or_zero();
    let rotation = if direction == Vec3::ZERO {
        Quat::IDENTITY
    } else {
        Quat::from_rotation_arc(Vec3::Z, direction)
    };
    let lifetime = Lifetime::default();
    let hit_once = HitOnce::default();
    bsn! {
        template_value(faction)
        template_value(PhysicalDamage(15.0))
        template_value(lifetime)
        template_value(hit_once)
        MeleeShape {
            range: 2.5,
            half_arc: {60.0_f32.to_radians()},
        }
        Transform {
            translation: {position},
            rotation: {rotation},
        }
        Mesh3d(asset_value(Cuboid::new(1.4, 0.15, 0.4)))
        MeshMaterial3d<StandardMaterial>(asset_value(StandardMaterial {
            base_color: Color::srgb(0.95, 0.95, 0.9),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }))
    }
}
