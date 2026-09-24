//! 近战横扫场景工厂：一次性攻击实体 + 扇形命中形状。

use bevy::prelude::*;

use crate::combat::attributes::{AttackFrame, InterruptPower, PhysicalDamage};
use crate::combat::components::Faction;
use crate::combat::lifecycle::{HitOnce, Lifetime};
use crate::combat::targeting::MeleeShape;

/// 近战横扫的伤害与属性（注册表 / HUD 展示也读这里，避免两处各写一份）。
pub const MELEE_DAMAGE: i32 = 15;
/// 速度帧（信息层读数）。
pub const MELEE_FRAME: u32 = 5;
/// 打断力度：命中时和目标的打断抗性掷骰对抗。
pub const MELEE_POWER: i32 = 3;

/// 近战横扫：短命攻击实体。
///
/// 命中由 [`detect_melee_system`](crate::combat::targeting::detect_melee_system) 负责，
/// 伤害走通用流水线，[`Lifetime`] 到期自动销毁。
///
/// `damage` 是**这一发的最终数值**（基础 `MELEE_DAMAGE` + 攻击者的武器加成），
/// 由执行器在生成时算好传进来——攻击实体自己不必知道"武器"是什么，
/// 下游（目标获取 / 命中管线 / 爆炸）也一行不用改。
pub fn melee_scene(position: Vec3, direction: Vec3, faction: Faction, damage: i32) -> impl Scene {
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
        template_value(PhysicalDamage(damage))
        template_value(AttackFrame(MELEE_FRAME))
        template_value(InterruptPower(MELEE_POWER))
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
