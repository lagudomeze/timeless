//! 战斗技能的静态定义：横扫 / 箭矢 / 火球 / 招架。
//!
//! **数值就在这里**（`MELEE_TIMING` / `FIREBALL_TIMING` / `PARRY_TIMING`…），
//! 本文件只把它们包成 [`AbilityDef`] 交给技能目录。
//! 移动 / 跳跃 / 翻滚走的是同一条路（见 [`crate::movement::abilities`]）。

use bevy::prelude::*;

use crate::skills::{
    AbilityCategory, AbilityDef, AbilityId, CombatTags, RegisterAbility, Requirement,
    TargetSelector,
};

use super::actions::{ARROW_TIMING, MELEE_TIMING};
use super::fireball::{FIREBALL_COST, FIREBALL_TIMING};
use super::melee::MELEE_DAMAGE;

/// 近战横扫：射程内一圈，能被打断也能被反制。
pub const MELEE_ABILITY: AbilityDef = AbilityDef {
    id: AbilityId::Melee,
    category: AbilityCategory::Attack,
    timing: MELEE_TIMING,
    targeting: TargetSelector::MeleeArc,
    cost: 0,
    // 免费技能：不需要精力，因此没有这一条
    requirements: &[],
    combat: CombatTags::STRIKE,
    power: MELEE_DAMAGE,
};

/// 箭矢：朝一个方向射出去。
pub const SHOOT_ABILITY: AbilityDef = AbilityDef {
    id: AbilityId::Shoot,
    category: AbilityCategory::Attack,
    timing: ARROW_TIMING,
    targeting: TargetSelector::Direction,
    cost: 0,
    // 免费技能：不需要精力，因此没有这一条
    requirements: &[],
    combat: CombatTags::STRIKE,
    power: MELEE_DAMAGE,
};

/// 火球：锁格 AoE，前摇最长、最贵。
pub const FIREBALL_ABILITY: AbilityDef = AbilityDef {
    id: AbilityId::Fireball,
    category: AbilityCategory::Spell,
    timing: FIREBALL_TIMING,
    targeting: TargetSelector::TargetCell,
    cost: FIREBALL_COST,
    requirements: &[Requirement::EnoughEnergy],
    combat: CombatTags::STRIKE,
    power: super::FIREBALL_DAMAGE,
};

/// 招架：**纯反应动作**——它需要「绑定的那次攻击」，因此不进一键菜单，
/// 但同样是一条定义（花费 / 节奏 / 标签都从目录读）。
pub const PARRY_ABILITY: AbilityDef = AbilityDef {
    id: AbilityId::Parry,
    category: AbilityCategory::Posture,
    timing: crate::combat::defense::PARRY_TIMING,
    targeting: TargetSelector::TargetEntity,
    cost: crate::combat::defense::PARRY_COST,
    requirements: &[Requirement::EnoughEnergy],
    combat: CombatTags::STRIKE,
    power: 0,
};

/// 战斗域交给技能目录的四条定义。
pub const ABILITIES: [AbilityDef; 4] = [
    MELEE_ABILITY,
    SHOOT_ABILITY,
    FIREBALL_ABILITY,
    PARRY_ABILITY,
];

/// 启动时把自己的定义交上去（写：[`crate::combat`]；消费：[`crate::skills`]）。
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
        assert_eq!(MELEE_ABILITY.timing, MELEE_TIMING);
        assert_eq!(FIREBALL_ABILITY.timing, FIREBALL_TIMING);
        assert_eq!(PARRY_ABILITY.timing, crate::combat::defense::PARRY_TIMING);
        assert_eq!(SHOOT_ABILITY.timing, ARROW_TIMING);
    }

    /// 招架不进一键菜单（它要绑一次攻击），但它仍然是一条技能。
    #[test]
    fn parry_is_a_definition_without_a_menu_slot() {
        let parry = PARRY_ABILITY;
        assert_eq!(parry.targeting, TargetSelector::TargetEntity);
        assert!(parry.cost > 0, "招架要花精力");
        assert_eq!(parry.category, AbilityCategory::Posture);
    }
}
