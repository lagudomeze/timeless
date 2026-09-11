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

/// 攻击射程，单位是**格**（决策层）。
///
/// 结算仍然用真实距离：判定时换算成 `AttackRange::world(self)` 再和距离比。
/// 这样设计稿里的「射程 1 格」与几何命中可以同时成立。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AttackRange(pub u32);

impl AttackRange {
    /// 近战：相邻一格。
    pub const MELEE: Self = Self(1);
    /// 远程：两格以内。
    pub const RANGED: Self = Self(2);

    /// 换算成世界距离（世界单位）。
    pub fn world(self) -> f32 {
        self.0 as f32 * crate::timeline::CELL_SIZE
    }
}

/// 速度帧：**越小越先命中**（攻击实体挂载）。
///
/// 无回合模型里的「帧」就是这一层排序权重——不是物理时钟。
/// 三层裁决的第一层，见 [`crate::combat::formula::resolve_combat`]。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackFrame(pub u32);

impl Default for AttackFrame {
    fn default() -> Self {
        Self(5)
    }
}

/// 破势：同时命中时打断对方（攻击实体挂载）。
///
/// 三层裁决的第三层。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Impact(pub u32);

impl Default for Impact {
    fn default() -> Self {
        Self(1)
    }
}
