//! 技能注册表：**展示与消耗的单一来源**。
//!
//! 菜单、HUD、可用性判断都读这里，因此「加一个技能」只需要加一条 [`SkillDef`]，
//! 不需要在三个地方各写一遍花费。数值先硬编码，后续外置成 `.ron`（见
//! [TODO.md](../../../TODO.md) 的「动作数值外置」）。

use crate::combat::attack::FIREBALL_DAMAGE;
use crate::combat::attack::actions::MELEE_TIMING;
use crate::combat::attack::fireball::{FIREBALL_COST, FIREBALL_FRAME, FIREBALL_TIMING};
use crate::combat::attack::melee::{MELEE_DAMAGE, MELEE_FRAME};
use crate::combat::defense::{PARRY_COST, ROLL_COST};
use crate::movement::ROLL_TIMING;
use crate::timeline::ActionTiming;

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
    pub timing: ActionTiming,
    /// 大致威力（展示用；实际伤害在各自的载荷里）
    pub power: i32,
    /// 速度帧（**展示用**："谁先动"的读数，见 `AttackFrame`）。
    /// 数值与载荷上挂的 `AttackFrame` 同源——加技能时两处都要有。
    pub frame: u32,
}

impl SkillDef {
    /// 这个菜单项对应目录里的哪一条定义。
    ///
    /// 「攻击」**没有**对应项：它是"贴脸近战、否则火球"的**派发规则**，
    /// 不是一条技能——所以这个函数回答 `Option`，条件校验遇 `None` 就退回用
    /// 自己的 `cost`（免费，因此永远可用）。
    pub fn catalogue_entry(&self) -> Option<crate::skills::AbilityId> {
        match self.kind {
            SkillKind::Attack => None, // 派发规则，不是技能
            SkillKind::Melee => Some(crate::skills::AbilityId::Melee),
            SkillKind::Fireball => Some(crate::skills::AbilityId::Fireball),
            SkillKind::Roll => Some(crate::skills::AbilityId::Roll),
        }
    }

    /// 此刻负担得起吗。
    ///
    /// **走 [`crate::skills::can_cast`]**：菜单的过滤与声明系统的校验必须是
    /// **同一条判据**，否则会出现"菜单里亮着、按下去被拒"这种两处各写一遍的经典漂移。
    /// 没有目录项的（「攻击」）用一个同形定义去问，效果一样而判据仍然只有一份。
    pub fn affordable(&self, stamina: u32) -> bool {
        crate::skills::can_cast(&self.as_ability(), stamina).is_ok()
    }

    /// 把菜单项看成一条技能定义（供条件校验用）。
    pub fn as_ability(&self) -> crate::skills::AbilityDef {
        let category = match self.kind {
            SkillKind::Attack | SkillKind::Melee => crate::skills::AbilityCategory::Attack,
            SkillKind::Fireball => crate::skills::AbilityCategory::Spell,
            SkillKind::Roll => crate::skills::AbilityCategory::Movement,
        };
        crate::skills::AbilityDef {
            // 「攻击」是派发规则，借近战的 id 只为走同一套校验（它免费）
            id: self
                .catalogue_entry()
                .unwrap_or(crate::skills::AbilityId::Melee),
            category,
            timing: self.timing,
            targeting: crate::skills::TargetSelector::SelfOnly,
            cost: self.cost,
            counter: None, // 菜单项本身不当反制；反制建议直接读目录里的定义
            requirements: if self.cost == 0 {
                &[]
            } else {
                &[crate::skills::Requirement::EnoughEnergy]
            },
            combat: crate::skills::CombatTags::STRIKE,
            power: self.power,
        }
    }
}

/// 技能表（顺序 = 菜单顺序 = 数字键 `1`~`5`）。
pub const SKILLS: [SkillDef; 4] = [
    SkillDef {
        kind: SkillKind::Attack,
        label: "attack",
        cost: FIREBALL_COST,
        timing: FIREBALL_TIMING,
        power: FIREBALL_DAMAGE,
        frame: FIREBALL_FRAME,
    },
    SkillDef {
        kind: SkillKind::Melee,
        label: "melee",
        cost: 0,
        timing: MELEE_TIMING,
        power: MELEE_DAMAGE,
        frame: MELEE_FRAME,
    },
    SkillDef {
        kind: SkillKind::Fireball,
        label: "fireball",
        cost: FIREBALL_COST,
        timing: FIREBALL_TIMING,
        power: FIREBALL_DAMAGE,
        frame: FIREBALL_FRAME,
    },
    SkillDef {
        kind: SkillKind::Roll,
        label: "roll",
        cost: ROLL_COST,
        timing: ROLL_TIMING,
        power: 0,
        // 翻滚不产生攻击实体，因此没有速度帧可言
        frame: 0,
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

/// 当前精力能负担得起的技能下标（菜单循环用）。
///
/// 判据与声明系统**同源**：[`SkillDef::ability`] 指向目录里的定义，
/// 菜单用 [`crate::skills::can_cast`] 问同一个问题——两处各写一遍
/// `cost <= stamina` 正是"菜单里亮着、按下去被拒"那种漂移的来源。
/// `menu_matches_the_catalogue` 这条测试钉住两边的数值不许分叉。
pub fn affordable_indices(stamina_current: u32) -> Vec<usize> {
    SKILLS
        .iter()
        .enumerate()
        .filter(|(_, def)| def.affordable(stamina_current))
        .map(|(index, _)| index)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::attack::melee::{MELEE_FRAME, MELEE_POWER};

    #[test]
    fn attack_and_melee_share_the_frame_of_their_payload() {
        let melee = SKILLS
            .iter()
            .find(|def| def.kind == SkillKind::Melee)
            .expect("注册表里应当有近战");
        assert_eq!(melee.power, MELEE_DAMAGE);
        assert_eq!(melee.timing, MELEE_TIMING);
        assert_eq!(MELEE_FRAME, 5, "注册表与载荷的帧数约定要保持一致");
        assert_eq!(MELEE_POWER, 3, "打断力度要与载荷约定一致");
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

    /// **菜单与目录不许分叉**：每个菜单项指到的那条定义，花费 / 节奏 / 威力
    /// 必须与菜单自己列的一致。
    ///
    /// 这是 `affordable` 走 `can_cast` 之后的对账——两边数值分叉时，
    /// 菜单会显示一个价、声明系统按另一个价校验。
    #[test]
    fn menu_matches_the_catalogue() {
        use crate::skills::{AbilityDef, AbilityId};

        // 目录是启动期注册的，测试里直接按各域的定义核对
        let catalogue: [(AbilityId, AbilityDef); 4] = [
            (
                AbilityId::Melee,
                crate::combat::attack::abilities::MELEE_ABILITY,
            ),
            (
                AbilityId::Shoot,
                crate::combat::attack::abilities::SHOOT_ABILITY,
            ),
            (
                AbilityId::Fireball,
                crate::combat::attack::abilities::FIREBALL_ABILITY,
            ),
            (AbilityId::Roll, crate::movement::abilities::ROLL_ABILITY),
        ];

        for def in SKILLS {
            let Some(id) = def.catalogue_entry() else {
                continue; // 「攻击」是派发规则，没有目录项
            };
            let entry = catalogue
                .iter()
                .find(|(candidate, _)| *candidate == id)
                .map(|(_, entry)| entry)
                .unwrap_or_else(|| panic!("目录里没有 {:?}", id));
            assert_eq!(def.cost, entry.cost, "{} 的花费与目录分叉了", def.label);
            assert_eq!(def.timing, entry.timing, "{} 的节奏与目录分叉了", def.label);
            assert_eq!(def.power, entry.power, "{} 的威力与目录分叉了", def.label);
        }
    }

    /// **速度帧不许分叉**：技能表里的 `frame` 与载荷上挂的 `AttackFrame` 必须一致。
    ///
    /// 它们本来是一份数据被抄到两处（技能表给 HUD 读数、载荷给攻击实体），
    /// 所以要有东西钉住——`docs/components.md` 记录过历史上正是这两处容易脱节。
    #[test]
    fn the_frame_matches_the_frame_on_the_payload() {
        assert_eq!(
            SKILLS
                .iter()
                .find(|def| def.kind == SkillKind::Melee)
                .map(|def| def.frame),
            Some(MELEE_FRAME),
            "近战技能表的 frame 与 `MELEE_FRAME` 分叉了"
        );
        assert_eq!(
            SKILLS
                .iter()
                .find(|def| def.kind == SkillKind::Fireball)
                .map(|def| def.frame),
            Some(FIREBALL_FRAME),
            "火球技能表的 frame 与 `FIREBALL_FRAME` 分叉了"
        );
        // 翻滚不产生攻击实体，帧数写 0 表示"没有读数"
        assert_eq!(
            SKILLS
                .iter()
                .find(|def| def.kind == SkillKind::Roll)
                .map(|def| def.frame),
            Some(0),
            "翻滚没有速度帧"
        );
    }
}
