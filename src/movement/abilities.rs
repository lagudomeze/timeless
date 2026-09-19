//! 移动域的技能定义：移动 / 跳跃 / 翻滚。
//!
//! **数值就在这里**（[`MOVE_TIMING`] / [`JUMP_TIMING`] / [`ROLL_TIMING`]），
//! 本文件只把它们包成 [`AbilityDef`] 交给技能目录——移动不是特例，
//! 它和火球、横扫一样只是"一种技能"（见 `docs/skills.md`）。

use bevy::prelude::*;

use crate::skills::{
    AbilityCategory, AbilityDef, AbilityId, CombatTags, RegisterAbility, TargetSelector,
};

use super::actions::{JUMP_TIMING, MOVE_TIMING, ROLL_TIMING};

/// 走一格：**没有消耗、几乎不设防**（走得快就容易被打断）。
pub const MOVE_ABILITY: AbilityDef = AbilityDef {
    id: AbilityId::Move,
    category: AbilityCategory::Movement,
    timing: MOVE_TIMING,
    targeting: TargetSelector::TargetCell,
    cost: 0,
    combat: CombatTags::STRIKE,
    power: 0,
};

/// 起跳：前摇最短、起手后不受打断（不给取消）。
pub const JUMP_ABILITY: AbilityDef = AbilityDef {
    id: AbilityId::Jump,
    category: AbilityCategory::Movement,
    timing: JUMP_TIMING,
    targeting: TargetSelector::SelfOnly,
    cost: 0,
    combat: CombatTags::COMMITTED,
    power: 0,
};

/// 翻滚：防御性位移，消耗 1 精力（花费登记在 `combat::defense`）。
pub const ROLL_ABILITY: AbilityDef = AbilityDef {
    id: AbilityId::Roll,
    category: AbilityCategory::Movement,
    timing: ROLL_TIMING,
    targeting: TargetSelector::SelfOnly,
    cost: crate::combat::defense::ROLL_COST,
    combat: CombatTags::COMMITTED,
    power: 0,
};

/// 移动域交给技能目录的三条定义。
pub const ABILITIES: [AbilityDef; 3] = [MOVE_ABILITY, JUMP_ABILITY, ROLL_ABILITY];

/// 启动时把自己的定义交上去（写：[`crate::movement`]；消费：[`crate::skills`]）。
pub fn register_abilities_system(mut registrations: MessageWriter<RegisterAbility>) {
    for def in ABILITIES {
        registrations.write(RegisterAbility(def));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 交上去的节奏必须就是载荷用的那份，不能各抄一遍。
    #[test]
    fn the_definitions_quote_the_payload_timings() {
        assert_eq!(MOVE_ABILITY.timing, MOVE_TIMING);
        assert_eq!(JUMP_ABILITY.timing, JUMP_TIMING);
        assert_eq!(ROLL_ABILITY.timing, ROLL_TIMING);

        let categories: Vec<AbilityCategory> = ABILITIES.iter().map(|def| def.category).collect();
        assert!(
            categories.iter().all(|c| *c == AbilityCategory::Movement),
            "移动域交上来的都是位移类技能"
        );
    }
}
