//! 移动域的技能定义：移动 / 跳跃 / 翻滚 / 冲刺。
//!
//! **数值就在这里**（[`MOVE_TIMING`] / [`JUMP_TIMING`] / [`ROLL_TIMING`] / [`DASH_TIMING`]），
//! 本文件只把它们包成 [`AbilityDef`] 交给技能目录——移动不是特例，
//! 它和火球、横扫一样只是"一种技能"（见 `docs/skills.md`）。

use bevy::prelude::*;

use crate::skills::{
    AbilityCategory, AbilityDef, AbilityId, CombatTags, CounterCost, RegisterAbility,
    TargetSelector,
};

use crate::config::ActionConfig;

use super::actions::{DASH_COST, DASH_TIMING, JUMP_TIMING, MOVE_TIMING, ROLL_TIMING};

/// 按配置生成四条定义（**数值来自 `.ron`**，缺省时等于下面的常量）。
///
/// `AbilityDef` 是 `Copy` 的纯数据，所以"从配置构造"是廉价的一次拷贝。
pub fn abilities_from(config: &ActionConfig) -> [AbilityDef; 4] {
    [
        AbilityDef {
            timing: config.move_.timing(),
            cost: crate::skills::ResourceCost::Energy(config.move_.cost),
            power: config.move_.power,
            ..MOVE_ABILITY
        },
        AbilityDef {
            timing: config.jump.timing(),
            cost: crate::skills::ResourceCost::Energy(config.jump.cost),
            power: config.jump.power,
            ..JUMP_ABILITY
        },
        AbilityDef {
            timing: config.roll.timing(),
            cost: crate::skills::ResourceCost::Energy(config.roll.cost),
            ..ROLL_ABILITY
        },
        AbilityDef {
            timing: config.dash.timing(),
            cost: crate::skills::ResourceCost::Energy(config.dash.cost),
            ..DASH_ABILITY
        },
    ]
}

/// 走一格：**没有消耗、几乎不设防**（走得快就容易被打断）。
pub const MOVE_ABILITY: AbilityDef = AbilityDef {
    id: AbilityId::Move,
    category: AbilityCategory::Movement,
    timing: MOVE_TIMING,
    targeting: TargetSelector::TargetCell,
    // 走一格免费：**但它仍然要求有精力**（见 `AbilityCategory::shared_requirement`）
    // ——"条件"与"花费"是两件事
    cost: crate::skills::ResourceCost::Energy(0),
    requirements: &[],
    combat: CombatTags::STRIKE,
    counter: None,
    power: 0,
};

/// 起跳：前摇最短、起手后不受打断（不给取消）。
pub const JUMP_ABILITY: AbilityDef = AbilityDef {
    id: AbilityId::Jump,
    category: AbilityCategory::Movement,
    timing: JUMP_TIMING,
    targeting: TargetSelector::SelfOnly,
    cost: crate::skills::ResourceCost::Energy(0),
    requirements: &[],
    combat: CombatTags::COMMITTED,
    counter: None,
    power: 0,
};

/// 翻滚：防御性位移，消耗 1 精力（花费登记在 `combat::defense`）。
pub const ROLL_ABILITY: AbilityDef = AbilityDef {
    id: AbilityId::Roll,
    category: AbilityCategory::Movement,
    timing: ROLL_TIMING,
    targeting: TargetSelector::SelfOnly,
    cost: crate::skills::ResourceCost::Energy(crate::combat::defense::ROLL_COST),
    // Movement 类的共享条件已是 EnoughEnergy（见 `AbilityCategory::shared_requirement`）
    requirements: &[],
    combat: CombatTags::COMMITTED,
    counter: Some(CounterCost::Free),
    power: 0,
};

/// 冲刺：朝一个方向冲两格（`Shift` + 方向键），消耗 1 精力。
///
/// **它是"用时间换距离"**：前摇比走一格重（0.25 vs 0.15），但一次跨两格——
/// 追人 / 脱离时用；贴身缠斗时不如走一格灵便。
/// 标签是 [`CombatTags::COMMITTED`]：蹬出去就收不回来（与跳跃 / 翻滚同一族）。
pub const DASH_ABILITY: AbilityDef = AbilityDef {
    id: AbilityId::Dash,
    category: AbilityCategory::Movement,
    timing: DASH_TIMING,
    targeting: TargetSelector::Direction,
    cost: crate::skills::ResourceCost::Energy(DASH_COST),
    // Movement 类的共享条件已是 EnoughEnergy（见 `AbilityCategory::shared_requirement`）
    requirements: &[],
    combat: CombatTags::COMMITTED,
    counter: None,
    power: 0,
};

/// 移动域交给技能目录的四条定义。
pub const ABILITIES: [AbilityDef; 4] = [MOVE_ABILITY, JUMP_ABILITY, ROLL_ABILITY, DASH_ABILITY];

/// 启动时把自己的定义交上去（写：[`crate::movement`]；消费：[`crate::skills`]）。
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
        assert_eq!(MOVE_ABILITY.timing, MOVE_TIMING);
        assert_eq!(JUMP_ABILITY.timing, JUMP_TIMING);
        assert_eq!(ROLL_ABILITY.timing, ROLL_TIMING);
        assert_eq!(DASH_ABILITY.timing, DASH_TIMING);

        let categories: Vec<AbilityCategory> = ABILITIES.iter().map(|def| def.category).collect();
        assert!(
            categories.iter().all(|c| *c == AbilityCategory::Movement),
            "移动域交上来的都是位移类技能"
        );
    }
}
