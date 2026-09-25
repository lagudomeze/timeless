//! 箭矢场景工厂：视觉 + 通用射弹数据。

use bevy::prelude::*;

use crate::combat::attributes::{AttackFrame, HitRadius, InterruptPower, PhysicalDamage};

/// 箭矢的速度帧（**信息层读数**；越小越快——箭最快）。
pub const ARROW_FRAME: u32 = 4;
/// 箭矢伤害（**单体**：与火球的 AoE 12 点相对——单体更高，这是两种投射物的分工）。
pub const ARROW_DAMAGE: i32 = 10;
/// 箭矢花多少**弹药**：单体、快，所以比重击（火球）便宜。
pub const ARROW_COST: u32 = 1;
/// 箭矢的打断力度：轻，但快（见 `ARROW_FRAME`）。
pub const ARROW_POWER: i32 = 1;
use crate::combat::components::Faction;
use crate::combat::lifecycle::Projectile;
use crate::movement::Velocity;

/// 箭速（世界单位 / 秒）：执行器用它算「射手要忙到什么时候」。
pub const ARROW_SPEED: f32 = 12.0;

/// 普通箭矢：穿透 1，命中一次即结束。
///
/// `position` / `direction` 由生成方现场计算；阵营随箭矢携带，供碰撞过滤
/// （不打自己人）。属性：伤害 `damage`（基础 + 射手武器加成）、帧 4（快）、
/// 打断力度 1（弱）——攻击实体自己不认识"武器"，加成由生成方算好传进来。
pub fn arrow_scene(position: Vec3, direction: Vec3, faction: Faction, damage: i32) -> impl Scene {
    let direction = direction.normalize_or_zero();
    let rotation = if direction == Vec3::ZERO {
        Quat::IDENTITY
    } else {
        Quat::from_rotation_arc(Vec3::Z, direction)
    };
    bsn! {
        template_value(faction)
        template_value(Velocity(direction * ARROW_SPEED))
        Projectile { max_hits: 1, current_hits: 0, finished: false }
        template_value(PhysicalDamage(damage))
        template_value(AttackFrame(ARROW_FRAME))
        template_value(InterruptPower(ARROW_POWER))
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
