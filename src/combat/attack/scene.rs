//! 攻击实体的**场景工厂**（只建实体与视觉，不裁决任何东西）。
//!
//! ## 为什么单独一个文件
//!
//! 这三个工厂是**唯一会用 `asset_value(...)` 现场造网格与材质的地方**——
//! 它们留在各自的载荷文件里时，`arrow.rs` / `melee.rs` / `fireball.rs` 会同时
//! 回答"这一手的数值是多少""它怎么飞""它长什么样"三个问题，而只有最后一个是渲染。
//! 载荷文件因此被迫引用 `Mesh3d` / `StandardMaterial`，逻辑域也不得不在插件里
//! `init_asset::<Mesh>()` 才能在测试中跑起来（见 `docs/backlog/dev.md`）。
//!
//! 分法照抄 [`equipment`](crate::equipment)：**载荷 / 声明 / 执行器留在各自的文件，
//! 场景工厂在这里**。它们与载荷是同一个领域的两半，所以引用载荷类型是正常的
//! （方向是 `scene ──▶ 载荷`，没有反向依赖）。
//!
//! ⚠️ 判断一个工厂该不该搬进来，看它**有没有渲染类型**：
//! [`shoot_action_scene`](super::actions::shoot_action_scene) /
//! [`melee_action_scene`](super::actions::melee_action_scene) 只挂载荷与节奏，
//! 没有任何 `Mesh3d`——它们留在 [`super::actions`] （搬过来只会多一次跳转）。
//!
//! ## 数值从哪来
//!
//! 工厂只负责"摆实体"：伤害 / 速度 / 帧这些**数值**由调用方（执行器）算好传进来，
//! 攻击实体自己不认识"装备"与"配置"。所以这里没有任何 `Res<ActionConfig>`
//! ——它是个纯函数式的实体工厂。

use bevy::prelude::*;

use crate::combat::attributes::{AttackFrame, HitRadius, InterruptPower, PhysicalDamage};
use crate::combat::components::Faction;
use crate::combat::lifecycle::{HitOnce, Lifetime, Projectile};
use crate::combat::reaction::TargetCell;
use crate::combat::targeting::MeleeShape;
use crate::movement::{Cell, Velocity};

use super::arrow::{ARROW_FRAME, ARROW_POWER, ARROW_SPEED};
use super::fireball::{FIREBALL_FRAME, FIREBALL_POWER, FIREBALL_RADIUS, FIREBALL_SPEED, Fireball};
use super::melee::{MELEE_FRAME, MELEE_POWER};

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

/// 一次近战横扫的攻击实体：**短命**、命中一次即结束、形状是正面 120° 扇形。
///
/// 命中由 [`detect_melee_system`](crate::combat::targeting::detect_melee_system) 负责，
/// 伤害走通用流水线，`Lifetime` 到期自动销毁。
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

/// 一颗飞行中的火球：锁目标格、带 AoE 半径与这一发的伤害。
///
/// 挂 [`TargetCell`]：飞行中的它同样构成威胁
/// （反应系统据此冻结世界，玩家还有机会躲开或者抢先把它打掉）。
///
/// `damage` 是**这一发的最终数值**（基础 `FIREBALL_DAMAGE` + 施法者的武器加成）；
/// [`Fireball::amount`] 会带着它一路走到爆炸结算——爆炸系统读的是那个组件，
/// 不需要认识"装备"。
pub fn fireball_scene(
    origin: Vec3,
    target_cell: Cell,
    faction: Faction,
    damage: i32,
) -> impl Scene {
    let target = target_cell.center();
    // 落到目标格中心正上方一点，避免贴地穿模
    let destination = Vec3::new(target.x, origin.y, target.y);
    let direction = (destination - origin).normalize_or_zero();
    let speed = if origin.distance(destination) > f32::EPSILON {
        FIREBALL_SPEED
    } else {
        0.0
    };
    let rotation = if direction == Vec3::ZERO {
        Quat::IDENTITY
    } else {
        Quat::from_rotation_arc(Vec3::Z, direction)
    };
    bsn! {
        template_value(faction)
        template_value(Velocity(direction * speed))
        Fireball {
            speed: FIREBALL_SPEED,
            amount: {damage},
            radius: FIREBALL_RADIUS,
        }
        TargetCell(target_cell)
        Projectile { max_hits: 0, current_hits: 0, finished: false }
        template_value(PhysicalDamage(damage))
        template_value(AttackFrame(FIREBALL_FRAME))
        template_value(InterruptPower(FIREBALL_POWER))
        HitRadius(0.35)
        Transform {
            translation: {origin},
            rotation: {rotation},
        }
        Mesh3d(asset_value(Sphere::new(0.35)))
        MeshMaterial3d<StandardMaterial>(asset_value(StandardMaterial {
            base_color: Color::srgb(1.0, 0.45, 0.15),
            emissive: LinearRgba::rgb(6.0, 1.2, 0.2),
            unlit: true,
            ..default()
        }))
    }
}
