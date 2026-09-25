//! 战斗技能的静态定义：横扫 / 箭矢 / 火球 / 招架。
//!
//! **数值就在这里**（`MELEE_TIMING` / `FIREBALL_TIMING` / `PARRY_TIMING`…），
//! 本文件只把它们包成 [`AbilityDef`] 交给技能目录。
//! 移动 / 跳跃 / 翻滚走的是同一条路（见 [`crate::movement::abilities`]）。

use bevy::prelude::*;

use crate::skills::{
    AbilityCategory, AbilityDef, AbilityId, CombatTags, CounterCost, RegisterAbility, Requirement,
    ResourceCost, TargetSelector,
};

use crate::combat::defense::PARRY_COST;
use crate::config::ActionConfig;

use super::actions::{ARROW_TIMING, MELEE_TIMING};
use super::arrow::{ARROW_COST, ARROW_DAMAGE};
use super::fireball::{FIREBALL_AMMO_COST, FIREBALL_TIMING};
use super::melee::MELEE_DAMAGE;

/// 近战横扫：射程内一圈，能被打断也能被反制。
pub const MELEE_ABILITY: AbilityDef = AbilityDef {
    id: AbilityId::Melee,
    category: AbilityCategory::Attack,
    timing: MELEE_TIMING,
    targeting: TargetSelector::MeleeArc,
    // **平 A 免费**：两条线都空了也永远还有事可做（分线的安全阀）
    cost: ResourceCost::Free,
    // 免费技能：不需要资源，因此没有这一条
    requirements: &[],
    combat: CombatTags::STRIKE,
    counter: None,
    power: MELEE_DAMAGE,
};

/// 箭矢：单体狙击，朝一个方向射出去。
///
/// ⚠️ `power` 必须等于**载荷真正的伤害**（`ARROW_DAMAGE`，见 `arrow.rs`）：
/// 这条曾经写成 `MELEE_DAMAGE`（15），而箭矢实际打 10——菜单接上箭矢之后，
/// `menu_matches_the_catalogue` 立刻把这个分叉抓了出来。
pub const SHOOT_ABILITY: AbilityDef = AbilityDef {
    id: AbilityId::Shoot,
    category: AbilityCategory::Attack,
    timing: ARROW_TIMING,
    targeting: TargetSelector::Direction,
    // 箭矢花**弹药**（远程线）
    cost: ResourceCost::Ammo(ARROW_COST),
    requirements: &[Requirement::EnoughAmmo],
    combat: CombatTags::STRIKE,
    counter: None,
    power: ARROW_DAMAGE,
};

/// 火球：锁格 AoE，前摇最长、最贵。
pub const FIREBALL_ABILITY: AbilityDef = AbilityDef {
    id: AbilityId::Fireball,
    category: AbilityCategory::Spell,
    timing: FIREBALL_TIMING,
    targeting: TargetSelector::TargetCell,
    // 火球是**重击**：花弹药（远程线），比箭矢更贵
    cost: ResourceCost::Ammo(FIREBALL_AMMO_COST),
    requirements: &[Requirement::EnoughAmmo],
    combat: CombatTags::STRIKE,
    counter: None,
    power: super::FIREBALL_DAMAGE,
};

/// 招架：**纯反应动作**——它需要「绑定的那次攻击」，因此不进一键菜单，
/// 但同样是一条定义（花费 / 节奏 / 标签都从目录读）。
pub const PARRY_ABILITY: AbilityDef = AbilityDef {
    id: AbilityId::Parry,
    category: AbilityCategory::Posture,
    timing: crate::combat::defense::PARRY_TIMING,
    targeting: TargetSelector::TargetEntity,
    cost: ResourceCost::Energy(crate::combat::defense::PARRY_COST),
    requirements: &[Requirement::EnoughEnergy],
    combat: CombatTags::COMMITTED,
    counter: Some(CounterCost::Resource(PARRY_COST)),
    power: 0,
};

/// 战斗域交给技能目录的四条定义。
pub const ABILITIES: [AbilityDef; 4] = [
    MELEE_ABILITY,
    SHOOT_ABILITY,
    FIREBALL_ABILITY,
    PARRY_ABILITY,
];

/// 按配置生成四条定义（**数值来自 `.ron`**，缺省时等于上面的常量）。
pub fn abilities_from(config: &ActionConfig) -> [AbilityDef; 4] {
    [
        AbilityDef {
            timing: config.melee.timing(),
            cost: ResourceCost::Free,
            power: config.melee.power,
            ..MELEE_ABILITY
        },
        AbilityDef {
            timing: config.shoot.timing(),
            cost: ResourceCost::Ammo(config.shoot.cost),
            power: config.shoot.power,
            requirements: if config.shoot.cost == 0 {
                &[]
            } else {
                &[Requirement::EnoughAmmo]
            },
            ..SHOOT_ABILITY
        },
        AbilityDef {
            timing: config.fireball.timing(),
            cost: ResourceCost::Ammo(config.fireball.cost),
            power: config.fireball.power,
            // 花不花钱决定要不要这条条件（与 `SkillDef::as_ability` 同一判据）
            requirements: if config.fireball.cost == 0 {
                &[]
            } else {
                &[Requirement::EnoughAmmo]
            },
            ..FIREBALL_ABILITY
        },
        AbilityDef {
            timing: config.parry.timing(),
            cost: ResourceCost::Energy(config.parry.cost),
            counter: Some(CounterCost::Resource(config.parry.cost)),
            ..PARRY_ABILITY
        },
    ]
}

/// 启动时把自己的定义交上去（写：[`crate::combat`]；消费：[`crate::skills`]）。
///
/// **从配置构造**：`config/actions.ron` 里的数值经此进入技能目录。
pub fn register_abilities_system(
    // 轻量测试 App 可能没装 `ConfigPlugin`：那时用内置默认值（= 各域常量）
    config: Option<Res<ActionConfig>>,
    mut registrations: MessageWriter<RegisterAbility>,
) {
    let config = config.map(|config| *config).unwrap_or_default();
    for def in abilities_from(&config) {
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
        assert!(parry.cost.amount() > 0, "招架要花精力");
        assert_eq!(parry.category, AbilityCategory::Posture);
    }
}
