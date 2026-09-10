//! 单位通用零件：玩家与敌人共用的一层。

use bevy::prelude::*;

use crate::combat::health::Health;
use crate::combat::{Collidable, Faction, HitRadius};
use crate::movement::Velocity;

/// 单位骨架：逻辑组件 + glTF 视觉根节点（`WorldAssetRoot`）。
///
/// 「玩家」和「怪物」都从这里出发，只在各自的工厂里追加**驱动源**
/// （输入 / AI）与特质（区块加载器 / 攻击冷却）——零件共用，驱动不同。
pub fn unit_scene(faction: Faction, position: Vec3, scale: f32, model: &'static str) -> impl Scene {
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
