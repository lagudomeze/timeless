//! 战斗数值属性组件。
//!
//! 每个属性独立成组件：伤害（输出侧）、护甲（防御侧）、命中半径（几何）、
//! 打断力度（反应侧）。不设「属性包」——新增机制就新增组件与专属系统，
//! 公式只读它需要的分量。
//!
//! **伤害是整数**：扣血、护甲减伤、反制伤害全是整数运算，因此没有浮点误差带来的
//! 「剩 0.0001 点血没死」这类边界问题。

use bevy::prelude::*;

/// 物理伤害（攻击实体挂载）：命中时扣多少血。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PhysicalDamage(pub i32);

/// 护甲（目标挂载）：物理伤害先扣它，最低 0。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Armor(pub i32);

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

/// 速度帧：出手快慢的排序权重（攻击实体挂载）。
///
/// 无回合模型里「到点」由 `ScheduledAction.execute_at` 决定，帧退居**信息层**：
/// 它不再参与结算，但仍是玩家判断「谁先动」的读数（见 TODO.md 的「洞察力」）。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackFrame(pub u32);

impl Default for AttackFrame {
    fn default() -> Self {
        Self(5)
    }
}

/// 打断力度（攻击实体挂载）：命中时用它和目标的**打断抗性**掷骰对抗。
///
/// `0` = 这一击不尝试打断（对抗由
/// [`interrupt_observer`](crate::timeline::interrupt_observer) 完成）。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InterruptPower(pub i32);
