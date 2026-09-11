//! 单位通用零件：玩家与敌人共用的一层。

use bevy::prelude::*;

use crate::combat::health::Health;
use crate::combat::{AttackRange, Collidable, Faction, HitRadius, Stamina};
use crate::movement::{Cell, Velocity};
use crate::timeline::Ready;

/// 单位骨架：逻辑组件 + glTF 视觉根节点（`WorldAssetRoot`）。
///
/// 「玩家」和「怪物」都从这里出发，只在各自的工厂里追加**驱动源**
/// （输入 / AI）与特质（区块加载器 / 攻击范围）——零件共用，驱动不同。
///
/// 带 [`Ready`] 出生：开局第一帧谁都可以决策（无回合模型没有「先规划」这一步）。
pub fn unit_scene(faction: Faction, position: Vec3, scale: f32, model: &'static str) -> impl Scene {
    let cell = Cell::from_world(position);
    let stamina = Stamina::default();
    bsn! {
        template_value(faction)
        template_value(Health::new(50.0))
        HitRadius(0.8)
        AttackRange::MELEE
        Collidable
        template_value(Velocity(Vec3::ZERO))
        template_value(Ready)
        template_value(stamina)
        template_value(cell)
        Transform {
            translation: {position},
            scale: {Vec3::splat(scale)},
        }
        WorldAssetRoot(model)
    }
}
