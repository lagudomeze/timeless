//! 技能栏的**场景**：一排图标槽位 + 悬停 tooltip 的 UI 夹具与节点标记组件。
//!
//! 槽位显示三件事：**图标**（占了什么位置）、**角标**（现在是精力消耗，等
//! `Cooldowns` 落地后同一个节点改显示剩余 CD）、**tooltip**（名称 / 消耗 / 前摇后摇 /
//! 威力）。外观怎么随状态变在 [`super::model`]，每帧写进去在 [`super::system`]。

use bevy::prelude::*;

use crate::combat::attack::SKILLS;

use super::super::{PANEL_BG, hud_text_tinted, load_ui_image};
use super::model::{SLOT_BG, SLOT_BORDER, icon_path};

/// 单个技能槽的边长。
pub const SLOT_SIZE: f32 = 56.0;
/// 槽位里图标的边长。
pub const ICON_SIZE: f32 = 38.0;

/// 槽位标记（`index` = 注册表下标）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct SkillSlot {
    pub index: usize,
}

/// 角标文本（消耗 / 将来的 CD）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct SkillBadge {
    pub index: usize,
}

/// tooltip 根（默认 `Display::None`）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct SkillTooltip;

/// tooltip 正文。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct SkillTooltipText;

/// 一个技能槽：图标 + 左上热键 + 右下角标。
fn skill_slot(font: &Handle<Font>, index: usize, icon: Handle<Image>) -> impl Bundle {
    (
        Name::new(format!("SkillSlot{index}")),
        Button,
        SkillSlot { index },
        Node {
            width: Val::Px(SLOT_SIZE),
            height: Val::Px(SLOT_SIZE),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(SLOT_BG),
        BorderColor::all(SLOT_BORDER),
        children![
            (
                Name::new(format!("SkillSlot{index}Icon")),
                Node {
                    width: Val::Px(ICON_SIZE),
                    height: Val::Px(ICON_SIZE),
                    ..default()
                },
                ImageNode {
                    image: icon,
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                },
            ),
            (
                Name::new(format!("SkillSlot{index}Hotkey")),
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(1.0),
                    left: Val::Px(4.0),
                    ..default()
                },
                hud_text_tinted(
                    font,
                    10.0,
                    format!("{}", index + 1),
                    Color::srgba(0.75, 0.80, 0.90, 0.9),
                ),
            ),
            (
                Name::new(format!("SkillSlot{index}Badge")),
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(1.0),
                    right: Val::Px(4.0),
                    ..default()
                },
                hud_text_tinted(font, 11.0, "", Color::WHITE),
                SkillBadge { index },
            ),
        ],
    )
}

/// 技能栏：底部居中的一排槽位 + 一个悬停 tooltip。
pub fn skill_bar(font: &Handle<Font>, assets: &AssetServer) -> impl Bundle {
    let icons: Vec<Handle<Image>> = SKILLS
        .iter()
        .map(|def| load_ui_image(assets, icon_path(def.kind)))
        .collect();
    (
        Name::new("SkillBar"),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(14.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::FlexEnd,
            ..default()
        },
        children![(
            Name::new("SkillBarPanel"),
            Node {
                padding: UiRect::all(Val::Px(6.0)),
                column_gap: Val::Px(8.0),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(PANEL_BG),
            children![
                skill_slot(font, 0, icons[0].clone()),
                skill_slot(font, 1, icons[1].clone()),
                skill_slot(font, 2, icons[2].clone()),
                skill_slot(font, 3, icons[3].clone()),
                (
                    Name::new("SkillTooltip"),
                    Node {
                        position_type: PositionType::Absolute,
                        bottom: Val::Px(SLOT_SIZE + 20.0),
                        left: Val::Px(0.0),
                        width: Val::Percent(100.0),
                        padding: UiRect::all(Val::Px(8.0)),
                        display: Display::None,
                        border_radius: BorderRadius::all(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.04, 0.05, 0.08, 0.95)),
                    SkillTooltip,
                    children![(
                        Name::new("SkillTooltipText"),
                        hud_text_tinted(font, 11.0, "", Color::srgb(0.92, 0.94, 0.98)),
                        SkillTooltipText,
                    )],
                ),
            ],
        )],
    )
}

/// 槽位池的大小必须与技能目录一致——池子是**按下标**建出来的，
/// 目录比池子长时多出来的技能永远没有槽位（表现为"少了一格"）。
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_slot_pool_covers_every_ability_in_the_catalogue() {
        assert_eq!(
            SKILLS.len(),
            4,
            "`skill_bar` 写死了 4 个槽位；目录变了就要同步改它，否则新技能没有槽位"
        );
    }
}
