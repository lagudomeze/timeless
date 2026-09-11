//! 近战横扫场景工厂：一次性攻击实体 + 扇形命中形状。

use bevy::prelude::*;

use crate::combat::attributes::{AttackFrame, Impact, PhysicalDamage};
use crate::combat::components::Faction;
use crate::combat::lifecycle::{HitOnce, Lifetime};
use crate::combat::targeting::MeleeShape;

/// 近战横扫的伤害与裁决参数（注册表 / HUD 展示也读这里，避免两处各写一份）。
pub const MELEE_DAMAGE: f32 = 15.0;
/// 速度帧：三层裁决的 L1。
pub const MELEE_FRAME: u32 = 5;
/// 破势：三层裁决的 L3。
pub const MELEE_IMPACT: u32 = 3;

/// 近战横扫：短命攻击实体。
///
/// 命中由 [`detect_melee_system`](crate::combat::targeting::detect_melee_system) 负责，
/// 伤害走通用流水线，[`Lifetime`] 到期自动销毁。
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
        template_value(PhysicalDamage(MELEE_DAMAGE))
        template_value(AttackFrame(MELEE_FRAME))
        template_value(Impact(MELEE_IMPACT))
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
