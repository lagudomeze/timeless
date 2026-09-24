//! 技能栏的**模型**：槽位该怎么显示（底色 / 描边 / 角标 / tooltip 文案）。
//!
//! 纯函数与纯数据：输入是"谁被选中、哪个槽被悬停、每个槽买不买得起"，
//! 输出是 `Color` 与 `String`。因此配色规则与 tooltip 文案可以脱离 App 直接单测。

use bevy::prelude::*;

use crate::combat::attack::{SKILLS, SkillDef, SkillKind};
use crate::combat::reaction::CounterSuggestion;

/// 技能图标贴图（见 `assets/LICENSES.md` 的来源与许可）。
///
/// **按 `AbilityId` 查**，不是按菜单项：`CLAUDE.md` 的约定是
/// "加技能 = 加一条 `AbilityDef` + 一张图标 PNG"，所以**每条技能**都该有图标，
/// 包括还没进菜单的（箭矢就是——它等着接输入）。
/// 只按 `SkillKind` 查的话，asset 验收就只覆盖菜单里的四个，
/// 新技能漏图标要等到它进菜单那天才被发现。
pub fn icon_path(kind: SkillKind) -> &'static str {
    match kind {
        SkillKind::Attack => "textures/ui/icon_attack.png",
        SkillKind::Melee => "textures/ui/icon_melee.png",
        SkillKind::Fireball => "textures/ui/icon_fireball.png",
        SkillKind::Roll => "textures/ui/icon_roll.png",
        // 「攻击」是派发规则、没有自己的图标；它显示成近战或火球那一格
        SkillKind::Shoot => "textures/ui/icon_shoot.png",
    }
}

/// 一个槽位此刻算不算「能拿来反制」。
///
/// 三态而不是布尔：**付不起的那条也要画出来**（只是画成另一副样子）——
/// 玩家看得见"我本可以用翻滚，但 Focus 不够"，比看不见更有信息量
/// （见 `docs/combat.md` 第四节）。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CounterHint {
    /// 不是反制建议（威胁窗口没开，或这一手不能当反制）
    #[default]
    None,
    /// 是反制建议且付得起 → 高亮
    Ready,
    /// 是反制建议但付不起 → 仍然标出来
    TooExpensive,
}

/// 技能栏快照：选中项 / 悬停项 / 每个槽位买不买得起 / 反制提示，
/// 四者都没变就整帧不碰 UI。
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SkillBarCache {
    pub selected: usize,
    pub hovered: Option<usize>,
    pub affordable: [bool; SKILLS.len()],
    pub counters: [CounterHint; SKILLS.len()],
}

/// 槽位底色：**反制优先**——威胁压过来时玩家要找的就是"我拿什么挡"，
/// 所以能当反制的那几手比"选中"更显眼。
pub fn slot_bg(affordable: bool, selected: bool, counter: CounterHint) -> Color {
    match counter {
        CounterHint::Ready => SLOT_BG_COUNTER,
        CounterHint::TooExpensive => SLOT_BG_COUNTER_BLOCKED,
        CounterHint::None if !affordable => SLOT_BG_LOCKED,
        CounterHint::None if selected => SLOT_BG_SELECTED,
        CounterHint::None => SLOT_BG,
    }
}

/// 槽位描边：悬停最优先，其次是"反制且付得起"，再是"选中的那一手且付得起"。
pub fn slot_border(hovered: bool, selected: bool, affordable: bool, counter: CounterHint) -> Color {
    if hovered {
        SLOT_BORDER_HOVER
    } else if counter == CounterHint::Ready {
        SLOT_BORDER_COUNTER
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

/// 每个槽位是不是反制建议（`None` = 没开窗口 / 没玩家）。
///
/// 靠 [`SkillDef::catalogue_entry`] 把菜单项映射回 `AbilityId` 再与建议比对——
/// 「攻击」是派发规则、没有目录项，因此永远不会被高亮（它确实不是一条技能）。
pub fn counter_hints(suggestions: Option<&[CounterSuggestion]>) -> [CounterHint; SKILLS.len()] {
    std::array::from_fn(|index| {
        let Some(ability) = SKILLS.get(index).and_then(SkillDef::catalogue_entry) else {
            return CounterHint::None;
        };
        let Some(suggestion) = suggestions
            .and_then(|list| list.iter().find(|suggestion| suggestion.ability == ability))
        else {
            return CounterHint::None;
        };
        if suggestion.affordable {
            CounterHint::Ready
        } else {
            CounterHint::TooExpensive
        }
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
        def.label().to_uppercase(),
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
/// 能当反制、且付得起：威胁压过来时最该看见的那一档。
const SLOT_BG_COUNTER: Color = Color::srgba(0.10, 0.30, 0.24, 0.94);
/// 能当反制但付不起：仍然标出来（"我本可以用它"也是信息）。
const SLOT_BG_COUNTER_BLOCKED: Color = Color::srgba(0.14, 0.11, 0.10, 0.92);
/// 槽位默认描边（场景建槽时用，运行时由 [`slot_border`] 换）。
pub const SLOT_BORDER: Color = Color::srgba(0.55, 0.60, 0.70, 0.85);
const SLOT_BORDER_SELECTED: Color = Color::srgb(0.95, 0.84, 0.42);
const SLOT_BORDER_HOVER: Color = Color::srgb(0.95, 0.96, 0.98);
/// 反制的描边（青色，与"选中"的金色区分开）。
const SLOT_BORDER_COUNTER: Color = Color::srgb(0.35, 0.95, 0.80);
const BADGE_TEXT: Color = Color::srgb(0.86, 0.90, 0.96);
const LOCKED_TEXT: Color = Color::srgb(0.62, 0.35, 0.35);

#[cfg(test)]
mod tests {
    use super::*;

    /// 三档底色互不相同，且买不起的那一档压过"选中"。
    #[test]
    fn the_three_slot_tints_are_distinct_and_locked_wins() {
        assert_ne!(
            slot_bg(true, false, CounterHint::None),
            slot_bg(true, true, CounterHint::None)
        );
        assert_ne!(
            slot_bg(true, false, CounterHint::None),
            slot_bg(false, false, CounterHint::None)
        );
        assert_eq!(
            slot_bg(false, true, CounterHint::None),
            slot_bg(false, false, CounterHint::None),
            "买不起时不该还显示选中色"
        );
    }

    /// 悬停压过选中：鼠标指哪一格，玩家看的就是哪一格。
    #[test]
    fn hovering_outranks_selection_for_the_border() {
        assert_eq!(
            slot_border(true, true, true, CounterHint::None),
            SLOT_BORDER_HOVER
        );
        assert_eq!(
            slot_border(false, true, true, CounterHint::None),
            SLOT_BORDER_SELECTED
        );
        assert_eq!(
            slot_border(false, false, true, CounterHint::None),
            SLOT_BORDER
        );
        assert_eq!(
            slot_border(false, true, false, CounterHint::None),
            SLOT_BORDER,
            "选中但付不起：不画金色描边"
        );
    }

    /// **反制提示压过"选中"**：威胁已经压过来了，玩家要找的是"我拿什么挡"，
    /// 而不是"我上一条选的是什么"。
    #[test]
    fn a_counter_suggestion_outranks_the_selected_slot() {
        assert_ne!(
            slot_bg(true, false, CounterHint::Ready),
            slot_bg(true, true, CounterHint::None),
            "能当反制的手不该与普通选中项同色"
        );
        assert_eq!(
            slot_border(false, true, true, CounterHint::Ready),
            SLOT_BORDER_COUNTER,
            "既是选中又能反制：画反制色（青色）而不是选中色（金色）"
        );
        assert_eq!(
            slot_border(true, true, true, CounterHint::Ready),
            SLOT_BORDER_HOVER,
            "但悬停仍然最优先"
        );
    }

    /// 付不起的反制**也要标出来**，只是用另一档底色——
    /// 玩家看得见"我本可以用它"，比看不见更有信息量。
    #[test]
    fn an_unaffordable_counter_is_still_drawn_differently() {
        let ready = slot_bg(true, false, CounterHint::Ready);
        let blocked = slot_bg(false, false, CounterHint::TooExpensive);
        let plain = slot_bg(true, false, CounterHint::None);
        assert_ne!(ready, blocked, "付得起与付不起必须可分辨");
        assert_ne!(blocked, plain, "付不起的反制仍要标出来，不能退回普通底色");
    }

    /// 建议列表 → 槽位提示：按 `AbilityId` 映射，**「攻击」永远不亮**
    /// （它是派发规则，不是一条技能）。
    #[test]
    fn counter_hints_map_suggestions_onto_slots() {
        use crate::skills::{AbilityId, CounterCost};

        let roll_index = SKILLS
            .iter()
            .position(|def| def.kind == SkillKind::Roll)
            .expect("技能栏里应当有翻滚");
        let attack_index = SKILLS
            .iter()
            .position(|def| def.kind == SkillKind::Attack)
            .expect("技能栏里应当有攻击");

        let hints = counter_hints(Some(&[CounterSuggestion {
            ability: AbilityId::Roll,
            cost: CounterCost::Free,
            affordable: true,
        }]));
        assert_eq!(hints[roll_index], CounterHint::Ready);
        assert_eq!(
            hints[attack_index],
            CounterHint::None,
            "「攻击」是派发规则、没有目录项，不该被当成反制"
        );

        // 付不起的照样标出来
        let broke = counter_hints(Some(&[CounterSuggestion {
            ability: AbilityId::Roll,
            cost: CounterCost::Resource(1),
            affordable: false,
        }]));
        assert_eq!(broke[roll_index], CounterHint::TooExpensive);

        // 没有窗口（`None`）时什么都不标
        assert!(
            counter_hints(None)
                .iter()
                .all(|hint| *hint == CounterHint::None),
            "没开窗口时不该有任何反制提示"
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
                SKILLS[index].label()
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
        assert!(text.contains(&def.label().to_uppercase()), "{text}");
        assert!(text.contains(&format!("cost {} EN", def.cost)), "{text}");
        assert!(
            text.contains(&format!("windup {:.2}s", def.timing.windup)),
            "{text}"
        );
        assert!(text.contains(&format!("power {:.0}", def.power)), "{text}");
    }
}
