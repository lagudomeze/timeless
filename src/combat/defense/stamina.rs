//! 精力：翻滚与招架的共用资源。
//!
//! 与弹药分线（弹药留给火球 / 重击）：精力是**防御与机动的货币**，
//! 每次恢复决策（后摇结束、决策槽重新变空）回一点，因此「一直滚」会把自己滚空。

use bevy::prelude::*;

/// 每次重新可决策时回复的精力。
pub const STAMINA_REGEN_PER_DECISION: u32 = 1;

/// 精力槽。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamina {
    pub current: u32,
    pub max: u32,
}

impl Default for Stamina {
    fn default() -> Self {
        Self::new(5)
    }
}

impl Stamina {
    /// 满精力单位。
    pub fn new(max: u32) -> Self {
        Self { current: max, max }
    }

    /// 够不够花。
    pub fn can_afford(&self, cost: u32) -> bool {
        self.current >= cost
    }

    /// 扣费；不够则不扣并返回 `false`（调用方据此放弃这个动作）。
    pub fn try_spend(&mut self, cost: u32) -> bool {
        if !self.can_afford(cost) {
            return false;
        }
        self.current -= cost;
        true
    }

    /// 回复（上限封顶）。
    pub fn regen(&mut self, amount: u32) {
        self.current = (self.current + amount).min(self.max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamina_spends_only_when_affordable() {
        let mut stamina = Stamina::new(2);
        assert!(stamina.try_spend(1));
        assert_eq!(stamina.current, 1);
        assert!(!stamina.try_spend(2), "不够时不该扣费");
        assert_eq!(stamina.current, 1, "失败的花费不改变状态");
    }

    #[test]
    fn stamina_regen_is_capped_at_max() {
        let mut stamina = Stamina::new(3);
        stamina.try_spend(2);
        stamina.regen(STAMINA_REGEN_PER_DECISION);
        stamina.regen(STAMINA_REGEN_PER_DECISION);
        stamina.regen(STAMINA_REGEN_PER_DECISION);
        assert_eq!(stamina.current, 3, "回复不得超过上限");
    }
}
