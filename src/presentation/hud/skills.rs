//! 技能栏：图标按钮 + 角标 + 悬停 tooltip。
//!
//! 槽位显示三件事：**图标**（占了什么位置）、**角标**（现在是精力消耗，等
//! `Cooldowns` 落地后同一个节点改显示剩余 CD）、**tooltip**（名称 / 消耗 / 前摇后摇 /
//! 威力）。不可负担的槽位整体压暗，选中的槽位加金色描边。

use bevy::prelude::*;

use crate::combat::Faction;
use crate::combat::defense::Stamina;
use crate::combat::skills::{MenuSelection, SKILLS, SkillKind};

use super::{HudCache, PANEL_BG, hud_text_tinted, load_ui_image};

/// 技能栏快照：选中项 / 悬停项 / 每个槽位买不买得起，三者都没变就整帧不碰 UI。
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SkillBarCache {
    selected: usize,
    hovered: Option<usize>,
    affordable: [bool; SKILLS.len()],
}

/// 单个技能槽的边长。
pub const SLOT_SIZE: f32 = 56.0;
/// 槽位里图标的边长。
pub const ICON_SIZE: f32 = 38.0;

const SLOT_BG: Color = Color::srgba(0.09, 0.11, 0.16, 0.88);
const SLOT_BG_SELECTED: Color = Color::srgba(0.32, 0.28, 0.12, 0.92);
const SLOT_BG_LOCKED: Color = Color::srgba(0.05, 0.05, 0.07, 0.9);
const SLOT_BORDER: Color = Color::srgba(0.55, 0.60, 0.70, 0.85);
const SLOT_BORDER_SELECTED: Color = Color::srgb(0.95, 0.84, 0.42);
const SLOT_BORDER_HOVER: Color = Color::srgb(0.95, 0.96, 0.98);
const LOCKED_TEXT: Color = Color::srgb(0.62, 0.35, 0.35);

/// 技能图标贴图（占位图，程序生成，见 `assets/LICENSES.md`）。
pub fn icon_path(kind: SkillKind) -> &'static str {
    match kind {
        SkillKind::Attack => "textures/ui/icon_attack.png",
        SkillKind::Melee => "textures/ui/icon_melee.png",
        SkillKind::Fireball => "textures/ui/icon_fireball.png",
        SkillKind::Roll => "textures/ui/icon_roll.png",
    }
}

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

/// 每帧刷新槽位外观、角标与 tooltip。
pub fn update_skill_bar_system(
    selection: Res<MenuSelection>,
    players: Query<(&Faction, &Stamina)>,
    mut cache: ResMut<HudCache>,
    mut slots: Query<(
        &SkillSlot,
        &Interaction,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
    mut badges: Query<(&SkillBadge, &mut Text, &mut TextColor), Without<SkillTooltipText>>,
    mut tooltips: Query<&mut Node, (With<SkillTooltip>, Without<SkillSlot>)>,
    mut tooltip_texts: Query<&mut Text, (With<SkillTooltipText>, Without<SkillBadge>)>,
) {
    let stamina = players
        .iter()
        .find(|(faction, _)| **faction == Faction::Player)
        .map(|(_, stamina)| stamina.current);
    let affordable: [bool; SKILLS.len()] = std::array::from_fn(|index| {
        SKILLS
            .get(index)
            .is_some_and(|def| stamina.is_none_or(|stamina| def.cost <= stamina))
    });

    // 悬停要读 `Interaction`（很便宜），但写节点前先比对快照
    let hovered = slots
        .iter()
        .find(|(_, interaction, _, _)| **interaction == Interaction::Hovered)
        .map(|(slot, _, _, _)| slot.index);
    let snapshot = SkillBarCache {
        selected: selection.index(),
        hovered,
        affordable,
    };
    if cache.skills == snapshot {
        return;
    }
    cache.skills = snapshot.clone();

    for (slot, interaction, mut background, mut border) in &mut slots {
        let affordable = snapshot.affordable[slot.index];
        let selected = slot.index == snapshot.selected;
        let is_hovered = *interaction == Interaction::Hovered;

        *background = BackgroundColor(if !affordable {
            SLOT_BG_LOCKED
        } else if selected {
            SLOT_BG_SELECTED
        } else {
            SLOT_BG
        });
        *border = BorderColor::all(if is_hovered {
            SLOT_BORDER_HOVER
        } else if selected && affordable {
            SLOT_BORDER_SELECTED
        } else {
            SLOT_BORDER
        });
    }

    for (badge, mut text, mut color) in &mut badges {
        let def = SKILLS.get(badge.index);
        let affordable = snapshot.affordable[badge.index];
        // 现在是「消耗」，CD 落地后这里改显示剩余秒数（同一个节点，同一套更新路径）
        **text = def.map(|def| def.cost.to_string()).unwrap_or_default();
        *color = TextColor(if affordable {
            Color::srgb(0.86, 0.90, 0.96)
        } else {
            LOCKED_TEXT
        });
    }

    for mut tooltip in &mut tooltips {
        tooltip.display = if hovered.is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Some(index) = snapshot.hovered {
        for mut text in &mut tooltip_texts {
            **text = tooltip_text(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<HudCache>()
            .insert_resource(MenuSelection::default())
            .add_systems(Update, update_skill_bar_system);
        app
    }

    fn spawn_slot(app: &mut App, index: usize) -> Entity {
        app.world_mut()
            .spawn((
                SkillSlot { index },
                Interaction::default(),
                BackgroundColor(SLOT_BG),
                BorderColor::all(SLOT_BORDER),
            ))
            .id()
    }

    /// tooltip 的显示状态（`Query::single` 需要 `&mut World`，所以这里收 `&mut App`）。
    fn tooltip_display(app: &mut App) -> Display {
        let mut query = app
            .world_mut()
            .query_filtered::<&Node, With<SkillTooltip>>();
        query.single(app.world_mut()).unwrap().display
    }

    #[test]
    fn affordable_and_selected_slots_get_distinct_tints() {
        let mut app = skill_app();
        let player = app
            .world_mut()
            .spawn((Faction::Player, Stamina::new(5)))
            .id();
        let melee = spawn_slot(&mut app, 1); // 免费技能
        let fireball = spawn_slot(&mut app, 2); // 2 精力
        let attack = spawn_slot(&mut app, 0); // 默认选中（2 精力）

        app.update();

        let background =
            |app: &App, entity: Entity| app.world().get::<BackgroundColor>(entity).unwrap().0;
        assert_ne!(
            background(&app, attack),
            background(&app, melee),
            "选中的槽位应当有独立底色"
        );
        assert_eq!(
            background(&app, melee),
            background(&app, fireball),
            "都没选中时用同一底色"
        );

        // 精力清零后，2 精力的技能整体压暗；免费的近战不变
        app.world_mut()
            .entity_mut(player)
            .get_mut::<Stamina>()
            .unwrap()
            .current = 0;
        app.update();
        assert_eq!(background(&app, fireball), SLOT_BG_LOCKED);
        assert_eq!(background(&app, melee), SLOT_BG, "免费技能永远可用");
    }

    #[test]
    fn badges_show_the_energy_cost() {
        let mut app = skill_app();
        let badge = app
            .world_mut()
            .spawn((
                SkillBadge { index: 2 },
                Text::new(""),
                TextColor(Color::WHITE),
            ))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<Text>(badge).unwrap().0,
            SKILLS[2].cost.to_string()
        );
    }

    #[test]
    fn hovering_a_slot_shows_its_tooltip() {
        let mut app = skill_app();
        let slot = spawn_slot(&mut app, 3);
        app.world_mut().spawn((SkillTooltip, Node::default()));
        let text = app
            .world_mut()
            .spawn((SkillTooltipText, Text::new("")))
            .id();

        app.update();
        assert_eq!(
            tooltip_display(&mut app),
            Display::None,
            "没悬停时 tooltip 应当隐藏"
        );

        *app.world_mut().get_mut::<Interaction>(slot).unwrap() = Interaction::Hovered;
        app.update();
        assert!(
            app.world().get::<Text>(text).unwrap().0.contains("ROLL"),
            "悬停应当显示该技能的说明"
        );
        assert_eq!(
            tooltip_display(&mut app),
            Display::Flex,
            "悬停时 tooltip 应当显示"
        );
    }

    /// 选中项 / 悬停 / 可负担性都没变时，技能栏一帧都不写。
    #[test]
    fn skill_bar_is_left_alone_when_nothing_changes() {
        let mut app = skill_app();
        let player = app
            .world_mut()
            .spawn((Faction::Player, Stamina::new(5)))
            .id();
        let slot = spawn_slot(&mut app, 2);

        app.update();
        // 拿哨兵色当探针：数据没变时系统不该把它盖回去
        app.world_mut().get_mut::<BackgroundColor>(slot).unwrap().0 = Color::WHITE;
        app.update();
        assert_eq!(
            app.world().get::<BackgroundColor>(slot).unwrap().0,
            Color::WHITE,
            "数据没变就不该重写槽位"
        );

        // 精力清零 → 2 精力的技能变不可负担，必须重画
        app.world_mut().get_mut::<Stamina>(player).unwrap().current = 0;
        app.update();
        assert_eq!(
            app.world().get::<BackgroundColor>(slot).unwrap().0,
            SLOT_BG_LOCKED
        );
    }
}
