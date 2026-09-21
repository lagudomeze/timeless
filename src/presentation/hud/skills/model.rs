//! 技能栏的**模型**：槽位该怎么显示（底色 / 描边 / 角标 / tooltip 文案）。
//!
//! 纯函数与纯数据：输入是"谁被选中、哪个槽被悬停、每个槽买不买得起"，
//! 输出是 `Color` 与 `String`。因此配色规则与 tooltip 文案可以脱离 App 直接单测。

use bevy::prelude::*;

use crate::combat::skills::{SKILLS, SkillKind};

/// 技能图标贴图（占位图，程序生成，见 `assets/LICENSES.md`）。
pub fn icon_path(kind: SkillKind) -> &'static str {
    match kind {
        SkillKind::Attack => "textures/ui/icon_attack.png",
        SkillKind::Melee => "textures/ui/icon_melee.png",
        SkillKind::Fireball => "textures/ui/icon_fireball.png",
        SkillKind::Roll => "textures/ui/icon_roll.png",
    }
}

/// 技能栏快照：选中项 / 悬停项 / 每个槽位买不买得起，三者都没变就整帧不碰 UI。
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SkillBarCache {
    pub selected: usize,
    pub hovered: Option<usize>,
    pub affordable: [bool; SKILLS.len()],
}

/// 槽位底色。
pub fn slot_bg(affordable: bool, selected: bool) -> Color {
    if !affordable {
        SLOT_BG_LOCKED
    } else if selected {
        SLOT_BG_SELECTED
    } else {
        SLOT_BG
    }
}

/// 槽位描边：悬停最优先，其次是"选中的那一手且付得起"。
pub fn slot_border(hovered: bool, selected: bool, affordable: bool) -> Color {
    if hovered {
        SLOT_BORDER_HOVER
    } else if selected && affordable {
        SLOT_BORDER_SELECTED
    } else {
        SLOT_BORDER
    }
}

/// 角标文字色：买不起时压暗。
pub fn badge_color(affordable: bool) -> Color {
    if affordable { BADGE_TEXT } else { LOCKED_TEXT }
}

/// 每个槽位此刻买不买得起（`None` = 场上没有玩家，按"都买得起"处理）。
pub fn affordability(stamina: Option<u32>) -> [bool; SKILLS.len()] {
    std::array::from_fn(|index| {
        SKILLS
            .get(index)
            .is_some_and(|def| stamina.is_none_or(|stamina| def.cost <= stamina))
    })
}

/// 角标内容：现在是「消耗」，CD 落地后这里改显示剩余秒数（同一个节点，同一套更新路径）。
pub fn badge_text(index: usize) -> String {
    SKILLS
        .get(index)
        .map(|def| def.cost.to_string())
        .unwrap_or_default()
}

/// tooltip 文本：名称 / 消耗 / 节奏 / 威力。
pub fn tooltip_text(index: usize) -> String {
    let Some(def) = SKILLS.get(index) else {
        return String::new();
    };
    format!(
        "{}   cost {} EN\nwindup {:.2}s   recovery {:.2}s\npower {:.0}",
        def.label.to_uppercase(),
        def.cost,
        def.timing.windup,
        def.timing.recovery,
        def.power
    )
}

/// 槽位默认底色（场景建槽时用，运行时由 [`slot_bg`] 换）。
pub const SLOT_BG: Color = Color::srgba(0.09, 0.11, 0.16, 0.88);
const SLOT_BG_SELECTED: Color = Color::srgba(0.32, 0.28, 0.12, 0.92);
/// 买不起的槽位底色。
pub const SLOT_BG_LOCKED: Color = Color::srgba(0.05, 0.05, 0.07, 0.9);
/// 槽位默认描边（场景建槽时用，运行时由 [`slot_border`] 换）。
pub const SLOT_BORDER: Color = Color::srgba(0.55, 0.60, 0.70, 0.85);
const SLOT_BORDER_SELECTED: Color = Color::srgb(0.95, 0.84, 0.42);
const SLOT_BORDER_HOVER: Color = Color::srgb(0.95, 0.96, 0.98);
const BADGE_TEXT: Color = Color::srgb(0.86, 0.90, 0.96);
const LOCKED_TEXT: Color = Color::srgb(0.62, 0.35, 0.35);

#[cfg(test)]
mod tests {
    use super::*;

    /// 三档底色互不相同，且买不起的那一档压过"选中"。
    #[test]
    fn the_three_slot_tints_are_distinct_and_locked_wins() {
        assert_ne!(slot_bg(true, false), slot_bg(true, true));
        assert_ne!(slot_bg(true, false), slot_bg(false, false));
        assert_eq!(
            slot_bg(false, true),
            slot_bg(false, false),
            "买不起时不该还显示选中色"
        );
    }

    /// 悬停压过选中：鼠标指哪一格，玩家看的就是哪一格。
    #[test]
    fn hovering_outranks_selection_for_the_border() {
        assert_eq!(slot_border(true, true, true), SLOT_BORDER_HOVER);
        assert_eq!(slot_border(false, true, true), SLOT_BORDER_SELECTED);
        assert_eq!(slot_border(false, false, true), SLOT_BORDER);
        assert_eq!(
            slot_border(false, true, false),
            SLOT_BORDER,
            "选中但付不起：不画金色描边"
        );
    }

    /// 免费技能永远可用；精力不够就没有任何一个槽是"白送"。
    #[test]
    fn affordability_follows_the_energy_cost() {
        let rich = affordability(Some(u32::MAX));
        assert!(rich.iter().all(|ok| *ok), "精力充足时全部可负担");

        let broke = affordability(Some(0));
        for (index, ok) in broke.iter().enumerate() {
            assert_eq!(
                *ok,
                SKILLS[index].cost == 0,
                "{} 的可用性应当只取决于它的消耗",
                SKILLS[index].label
            );
        }

        let none = affordability(None);
        assert!(none.iter().all(|ok| *ok), "场上没玩家时不做灰化");
    }

    /// 越界的槽位读数是空的（池子比目录大时不该 panic）。
    #[test]
    fn out_of_range_slots_read_empty() {
        assert_eq!(tooltip_text(SKILLS.len()), "");
        assert_eq!(badge_text(SKILLS.len()), "");
    }

    /// tooltip 要带上玩家真正要判断的三个数：消耗、前后摇、威力。
    #[test]
    fn the_tooltip_carries_cost_timing_and_power() {
        let def = &SKILLS[0];
        let text = tooltip_text(0);
        assert!(text.contains(&def.label.to_uppercase()), "{text}");
        assert!(text.contains(&format!("cost {} EN", def.cost)), "{text}");
        assert!(
            text.contains(&format!("windup {:.2}s", def.timing.windup)),
            "{text}"
        );
        assert!(text.contains(&format!("power {:.0}", def.power)), "{text}");
    }
}
