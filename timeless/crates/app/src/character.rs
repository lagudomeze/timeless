//! # 单位场景工厂（BSN 组合逻辑组件 + 视觉）
//!
//! 玩家 / 敌人是同一套「逻辑小组件 + 视觉根节点」的两种参数化场景：
//! 阵营、血量、命中半径、可碰撞、速度都直接声明在场景里，不再由 setup
//! 手动拼长元组 spawn。

use bevy::prelude::*;

use crate::combat::{Collidable, Faction, HitRadius, Velocity};
use crate::health::Health;

/// 玩家单位：示例用 glTF 岩石占位（后续替换角色模型）
pub fn player() -> impl Scene {
    unit(
        Faction::Player,
        Vec3::new(2.0, 0.0, 2.0),
        3.0,
        "models/nature/rock_largeA.glb#Scene0",
    )
}

/// 敌人单位：示例用 glTF 树木占位（后续替换怪物模型）
pub fn enemy() -> impl Scene {
    unit(
        Faction::Enemy,
        Vec3::new(7.0, 0.0, 7.0),
        1.6,
        "models/nature/tree_oak.glb#Scene0",
    )
}

/// 单位场景模板：根实体承载逻辑组件，glTF 作为视觉子节点由 `WorldAssetRoot` 挂载。
fn unit(faction: Faction, position: Vec3, scale: f32, model: &'static str) -> impl Scene {
    bsn! {
        template_value(faction)
        template_value(Health::new(50.0))
        HitRadius(0.8)
        Collidable
        template_value(Velocity(Vec3::ZERO))
        Transform {
            translation: {position},
            scale: {Vec3::splat(scale)},
        }
        WorldAssetRoot(model)
    }
}
