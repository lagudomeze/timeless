//! 技能栏的**场景**：一排图标槽位 + 悬停 tooltip 的 UI 夹具与节点标记组件。
//!
//! 槽位显示三件事：**图标**（占了什么位置）、**角标**（现在是精力消耗，等
//! `Cooldowns` 落地后同一个节点改显示剩余 CD）、**tooltip**（名称 / 消耗 / 前摇后摇 /
//! 威力）。外观怎么随状态变在 [`super::model`]，每帧写进去在 [`super::system`]。

use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};

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

/// 悬停 tooltip（默认隐藏）：名称 / 消耗 / 前后摇 / 威力，绝对定位在槽位上方。
fn tooltip(font: &Handle<Font>) -> impl Bundle {
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
    )
}

/// 技能栏：底部居中的一排槽位 + 一个悬停 tooltip。
///
/// ⚠️ **槽位数量由 `SKILLS` 决定**，不再手写一列 `skill_slot(font, 0.., icon)`：
/// 手写的那版在目录从 4 条加到 5 条时**静默少一格**——`SkillSlot` 池子只有 4 个，
/// 表现为"第 5 个技能按得出来（条码 / 声明都走通了），但栏里根本没有那一格"。
/// `children!` 只吃字面量列表，所以槽位用 [`SpawnWith`]（迭代建子实体）挂。
pub fn skill_bar(font: &Handle<Font>, assets: &AssetServer) -> impl Bundle {
    let icons: Vec<Handle<Image>> = SKILLS
        .iter()
        .map(|def| load_ui_image(assets, icon_path(def.kind)))
        .collect();
    let font = font.clone();
    let slots_font = font.clone();
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
            // 吃掉指针：整个技能栏是一块可点的区域（槽位本身是 `Button`，
            // 已是 `Block`；这里管的是槽位之间的空隙）
            FocusPolicy::Block,
            RelativeCursorPosition::default(),
            Node {
                padding: UiRect::all(Val::Px(6.0)),
                column_gap: Val::Px(8.0),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(PANEL_BG),
            // 槽位是**迭代建**的：目录几条就几个槽。
            // `children!` 只吃字面量列表，装不下"按 `SKILLS` 数量生成"这件事，
            // 所以这里用 `SpawnWith` 逐个 spawn。**它自己就提供 `Children`**，
            // 因此面板里不能再写 `children![…]`（两处会撞同一个组件）。
            // tooltip 因此包一层节点挂在**槽位层外面**（它绝对定位，位置不变）。
            Children::spawn(SpawnWith(move |parent: &mut ChildSpawner| {
                for (index, icon) in icons.iter().enumerate() {
                    parent.spawn(skill_slot(&slots_font, index, icon.clone()));
                }
                parent.spawn(tooltip(&font));
            })),
        )],
    )
}

/// 槽位池的大小必须与技能目录一致——池子是**按下标**建出来的，
/// 目录比池子长时多出来的技能永远没有槽位（表现为"少了一格"）。
#[cfg(test)]
mod tests {
    use super::*;

    /// **建出来的栏必须每一条技能都有一格**。
    ///
    /// 这条曾经是空跑的：它只断言 `SKILLS.len() == 4`，而 `skill_bar` 里**手写**了
    /// 四个 `skill_slot(font, 0..3, ..)`——目录加到 5 条时它照样绿，实际却少一格
    /// （第 5 个技能按得出来、栏里看不见）。改为测**真实的 `SkillSlot` 实体数**。
    #[test]
    fn the_bar_builds_one_slot_per_ability() {
        // 只建实体、**不装 `UiPlugin`**：那需要一整套 UI 资源（`HoverMap` 等），
        // 而这条要断言的是"`SpawnWith` 真的建了几格"，与布局无关。
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Image>();
        let assets = app.world().resource::<AssetServer>().clone();
        let font = Handle::<Font>::default();

        app.world_mut().spawn(skill_bar(&font, &assets));
        app.world_mut().flush(); // `Children::spawn` 要等命令落地才建子实体

        let mut query = app.world_mut().query::<&SkillSlot>();
        let mut indices: Vec<usize> = query.iter(app.world()).map(|slot| slot.index).collect();
        indices.sort_unstable();
        assert_eq!(
            indices,
            (0..SKILLS.len()).collect::<Vec<_>>(),
            "每一条技能都要有自己的一格（少了就是手写槽位没跟着目录改）"
        );
    }
}
