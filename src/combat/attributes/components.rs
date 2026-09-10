//! 战斗数值属性组件。
//!
//! 每个属性独立成组件：伤害（输出侧）、护甲（防御侧）、命中半径（几何）。
//! 不设「属性包」——新增机制就新增组件与专属系统，公式只读它需要的分量。

use bevy::prelude::*;

/// 物理伤害值（攻击实体挂载）。
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct PhysicalDamage(pub f32);

/// 护甲（目标挂载，物理伤害先扣护甲）。
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Armor(pub f32);

/// 命中半径（射弹与目标各有一个；距离 ≤ 两者半径之和即接触）。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct HitRadius(pub f32);

impl Default for HitRadius {
    fn default() -> Self {
        Self(0.5)
    }
}
