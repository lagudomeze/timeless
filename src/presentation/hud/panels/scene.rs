//! 单位面板的**场景**：头像 + 名字 / 状态行 + HP / EN 条的 UI 夹具与节点标记组件。
//!
//! 只**建**实体，不读游戏状态——每帧把读数写进去的是 [`super::system`]。

use bevy::prelude::*;

use crate::combat::Faction;

use super::super::actions::ActionLabel;
use super::super::{
    EN_COLOR, HP_COLOR, PANEL_BG, TRACK_BG, faction_color, hud_text, hud_text_tinted,
};

/// 面板整体尺寸（像素，还会被 `UiScale` 缩放）。
pub const PANEL_WIDTH: f32 = 340.0;
/// 见 [`PANEL_WIDTH`]。
pub const PANEL_HEIGHT: f32 = 104.0;
/// 头像边长。
pub const PORTRAIT_SIZE: f32 = 64.0;

/// 面板根标记。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct UnitPanel {
    pub faction: Faction,
}

/// 条本体（改宽度）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub enum PanelBar {
    Hp(Faction),
    En(Faction),
}

/// 面板文本：血量 / 精力 / 状态行。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub enum PanelText {
    Hp(Faction),
    En(Faction),
    State(Faction),
}

/// 阵营 → 实体名前缀（`PlayerPanel` / `EnemyPanel`）。
pub fn faction_prefix(faction: Faction) -> &'static str {
    match faction {
        Faction::Player => "Player",
        Faction::Enemy => "Enemy",
    }
}

/// 一格条（轨道 + 填充 + 居中文本）。
pub fn status_bar(font: &Handle<Font>, bar: PanelBar, color: Color) -> impl Bundle {
    // `PlayerHp` / `EnemyEn`：轨道、填充、文本三种节点共用这个前缀
    let (label, name) = match bar {
        PanelBar::Hp(faction) => (
            PanelText::Hp(faction),
            format!("{}Hp", faction_prefix(faction)),
        ),
        PanelBar::En(faction) => (
            PanelText::En(faction),
            format!("{}En", faction_prefix(faction)),
        ),
    };
    (
        Name::new(format!("{name}Bar")),
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(16.0),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(TRACK_BG),
        children![
            (
                Name::new(format!("{name}Fill")),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(color),
                bar,
            ),
            (
                Name::new(format!("{name}Text")),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                hud_text_tinted(font, 11.0, "", Color::srgb(0.95, 0.96, 0.98)),
                label,
            ),
        ],
    )
}

/// 一个单位面板：头像 + 名字 / 状态行 + HP / EN 条。
pub fn unit_panel(font: &Handle<Font>, faction: Faction, portrait: Handle<Image>) -> impl Bundle {
    let color = faction_color(faction);
    let title = match faction {
        Faction::Player => "PLAYER",
        Faction::Enemy => "ENEMY",
    };
    // 敌人面板镜像：头像贴右边
    let direction = match faction {
        Faction::Player => FlexDirection::Row,
        Faction::Enemy => FlexDirection::RowReverse,
    };
    let (left, right) = match faction {
        Faction::Player => (Val::Px(14.0), Val::Auto),
        Faction::Enemy => (Val::Auto, Val::Px(14.0)),
    };
    let prefix = faction_prefix(faction);
    (
        Name::new(format!("{prefix}Panel")),
        UnitPanel { faction },
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(14.0),
            left,
            right,
            width: Val::Px(PANEL_WIDTH),
            height: Val::Px(PANEL_HEIGHT),
            padding: UiRect::all(Val::Px(8.0)),
            column_gap: Val::Px(10.0),
            flex_direction: direction,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(PANEL_BG),
        BorderColor::all(color),
        children![
            (
                Name::new(format!("{prefix}Portrait")),
                Node {
                    width: Val::Px(PORTRAIT_SIZE),
                    height: Val::Px(PORTRAIT_SIZE),
                    padding: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(TRACK_BG),
                BorderColor::all(color),
                ImageNode {
                    image: portrait,
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                },
            ),
            (
                Name::new(format!("{prefix}Info")),
                Node {
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(3.0),
                    ..default()
                },
                children![
                    (
                        Name::new(format!("{prefix}StateLine")),
                        hud_text(font, 13.0, format!("{title} · …")),
                        PanelText::State(faction),
                    ),
                    status_bar(font, PanelBar::Hp(faction), HP_COLOR),
                    status_bar(font, PanelBar::En(faction), EN_COLOR),
                    (
                        Name::new(format!("{prefix}Action")),
                        hud_text(font, 11.0, "act: -"),
                        ActionLabel { faction },
                    ),
                ],
            ),
        ],
    )
}
