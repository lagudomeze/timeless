//! 技能注册表：**展示与消耗的单一来源**。
//!
//! 菜单、HUD、可用性判断都读这里，因此「加一个技能」只需要加一条 [`SkillDef`]，
//! 不需要在三个地方各写一遍花费。数值先硬编码，后续外置成 `.ron`（见
//! [TODO.md](../../../TODO.md) 的「动作数值外置」）。

use crate::combat::defense::{PARRY_COST, ROLL_COST};
use crate::combat::skills::FIREBALL_DAMAGE;
use crate::combat::skills::fireball::FIREBALL_COST;
use crate::combat::skills::melee::MELEE_DAMAGE;
use crate::timeline::timing;

/// 一件**有前置条件**的技能（需要消耗、需要距离）。
///
/// 跳跃 / 移动 / 招架这类无消耗或不依赖目标的动作不在注册表里——它们由按键直接触发。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SkillKind {
    /// 攻击：贴脸时横扫、否则扔火球（由 `use_selected_skill_system` 按距离派发）
    #[default]
    Attack,
    /// 近战横扫（固定按 E 也能用）
    Melee,
    /// 火球（锁格 AoE）
    Fireball,
    /// 翻滚（远离威胁退一格 + 无敌帧）
    Roll,
}

impl SkillKind {
    /// HUD / 日志用的英文短名。
    pub fn label(self) -> &'static str {
        match self {
            Self::Attack => "attack",
            Self::Melee => "melee",
            Self::Fireball => "fireball",
            Self::Roll => "roll",
        }
    }
}

/// 注册表条目。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkillDef {
    pub kind: SkillKind,
    pub label: &'static str,
    /// 精力消耗
    pub cost: u32,
    /// 动作节奏（前摇 / 后摇），HUD 展示用
    pub timing: timing::ActionTiming,
    /// 大致威力（展示用；实际伤害在各自的载荷里）
    pub power: f32,
}

/// 技能表（顺序 = 菜单顺序 = 数字键 `1`~`5`）。
pub const SKILLS: [SkillDef; 4] = [
    SkillDef {
        kind: SkillKind::Attack,
        label: "attack",
        cost: FIREBALL_COST,
        timing: timing::SHOOT,
        power: FIREBALL_DAMAGE,
    },
    SkillDef {
        kind: SkillKind::Melee,
        label: "melee",
        cost: 0,
        timing: timing::MELEE,
        power: MELEE_DAMAGE,
    },
    SkillDef {
        kind: SkillKind::Fireball,
        label: "fireball",
        cost: FIREBALL_COST,
        timing: timing::SHOOT,
        power: FIREBALL_DAMAGE,
    },
    SkillDef {
        kind: SkillKind::Roll,
        label: "roll",
        cost: ROLL_COST,
        timing: timing::ROLL,
        power: 0.0,
    },
];

/// 招架不在 [`SKILLS`] 里（它需要「绑定的那次攻击」，是纯反应动作），
/// 但消耗在这里登记，避免菜单与防御域各写一份。
pub const PARRY_COST_DISPLAY: u32 = PARRY_COST;

/// 按索引取条目。
pub fn skill(index: usize) -> Option<&'static SkillDef> {
    SKILLS.get(index)
}

/// 按种类取索引（`None` = 不在一键可达的列表里，例如招架）。
pub fn index_of(kind: SkillKind) -> Option<usize> {
    SKILLS.iter().position(|def| def.kind == kind)
}

/// 当前精力能负担得起的技能下标。
pub fn affordable_indices(stamina_current: u32) -> Vec<usize> {
    SKILLS
        .iter()
        .enumerate()
        .filter(|(_, def)| def.cost <= stamina_current)
        .map(|(index, _)| index)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::skills::melee::{MELEE_FRAME, MELEE_IMPACT};

    #[test]
    fn attack_and_melee_share_the_frame_of_their_payload() {
        let melee = SKILLS
            .iter()
            .find(|def| def.kind == SkillKind::Melee)
            .expect("注册表里应当有近战");
        assert_eq!(melee.power, MELEE_DAMAGE);
        assert_eq!(melee.timing, timing::MELEE);
        assert_eq!(MELEE_FRAME, 5, "注册表与载荷的帧数约定要保持一致");
        assert_eq!(MELEE_IMPACT, 3, "破势要与三层裁决的 L3 约定一致");
    }

    #[test]
    fn affordability_filters_by_cost() {
        assert_eq!(affordable_indices(0), vec![1], "0 精力时只有免费的近战可选");
        assert_eq!(affordable_indices(1), vec![1, 3], "1 点精力够翻滚");
        assert_eq!(
            affordable_indices(5),
            vec![0, 1, 2, 3],
            "满精力四个技能全开"
        );
    }

    #[test]
    fn every_listed_skill_has_a_stable_index() {
        for (index, def) in SKILLS.iter().enumerate() {
            assert_eq!(index_of(def.kind), Some(index));
        }
        assert_eq!(index_of(SkillKind::Attack), Some(0));
    }
}
