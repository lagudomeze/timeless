//! 单位面板的**场景**：头像 + 名字 / 状态行 + HP / EN 条的 UI 夹具与节点标记组件。
//!
//! 只**建**实体，不读游戏状态——每帧把读数写进去的是 [`super::system`]。

use bevy::prelude::*;

use crate::combat::Faction;

use super::super::actions::ActionLabel;
use super::super::{
    EN_COLOR, HP_COLOR, PANEL_BG, TRACK_BG, faction_color, hud_text, hud_text_tinted,
};
use super::model::PanelSlot;

/// 面板整体尺寸（像素，还会被 `UiScale` 缩放）。
pub const PANEL_WIDTH: f32 = 340.0;
/// 见 [`PANEL_WIDTH`]。
pub const PANEL_HEIGHT: f32 = 104.0;
/// 头像边长。
pub const PORTRAIT_SIZE: f32 = 64.0;

/// 面板根标记（玩家一格；敌人**每行一格**，见 [`unit_row`]）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct UnitPanel {
    pub slot: PanelSlot,
}

/// 条本体（改宽度）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub enum PanelBar {
    Hp(PanelSlot),
    En(PanelSlot),
}

/// 面板文本：血量 / 精力 / 状态行。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub enum PanelText {
    Hp(PanelSlot),
    En(PanelSlot),
    State(PanelSlot),
    /// **洞察力读数**（`docs/insight.md` 第四节）：射程 / 打断抗性 / 战术。
    /// 只有敌人格有内容（玩家看自己的面板不需要"我够得到多远"）。
    Insight(PanelSlot),
}

/// 一格的实体名前缀（`PlayerPanel` / `Enemy1Panel`）。
///
/// 名字里带名次：BRP 排查"第二个敌人那一行为什么是空的"时要能一眼找到节点。
pub fn slot_prefix(slot: PanelSlot) -> String {
    match slot {
        PanelSlot::Player => "Player".to_string(),
        PanelSlot::Enemy(index) => format!("Enemy{}", index + 1),
    }
}

/// 一格条（轨道 + 填充 + 居中文本）。
pub fn status_bar(font: &Handle<Font>, bar: PanelBar, color: Color) -> impl Bundle {
    // `PlayerHp` / `Enemy1En`：轨道、填充、文本三种节点共用这个前缀
    let (label, name) = match bar {
        PanelBar::Hp(slot) => (PanelText::Hp(slot), format!("{}Hp", slot_prefix(slot))),
        PanelBar::En(slot) => (PanelText::En(slot), format!("{}En", slot_prefix(slot))),
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

/// 一格的「状态行 + HP / EN 条 + 当前行动」——**三种面板共用**这一个内容块。
///
/// 玩家面板与敌人行**内容完全一样**，只是摆放位置与头像有无不同；抽成一处
/// 就不会出现"玩家面板加了护甲读数、敌人行忘了加"这种漂移。
fn slot_content(font: &Handle<Font>, slot: PanelSlot) -> impl Bundle {
    let prefix = slot_prefix(slot);
    let faction = slot.faction();
    (
        Name::new(format!("{prefix}Info")),
        Node {
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            min_width: Val::Px(0.0),
            row_gap: Val::Px(2.0),
            ..default()
        },
        children![
            (
                Name::new(format!("{prefix}StateLine")),
                hud_text(font, 13.0, "…"),
                PanelText::State(slot),
            ),
            status_bar(font, PanelBar::Hp(slot), HP_COLOR),
            status_bar(font, PanelBar::En(slot), EN_COLOR),
            (
                Name::new(format!("{prefix}Action")),
                hud_text(font, 11.0, "act: -"),
                ActionLabel { faction },
            ),
            // **洞察力读数**（`docs/insight.md` 第四节）：只在**敌人**行上挂——
            // 玩家面板那一格读自己就够了，多一行只会把固定的 `PANEL_HEIGHT` 挤紧。
            (
                Name::new(format!("{prefix}InsightLine")),
                hud_text_tinted(font, 11.0, "", Color::srgb(0.72, 0.80, 0.92)),
                PanelText::Insight(slot),
            ),
        ],
    )
}

/// 玩家面板：头像 + 内容块，常驻左下。
pub fn unit_panel(font: &Handle<Font>, portrait: Handle<Image>) -> impl Bundle {
    let slot = PanelSlot::Player;
    let color = faction_color(Faction::Player);
    (
        Name::new("PlayerPanel"),
        UnitPanel { slot },
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(14.0),
            left: Val::Px(14.0),
            width: Val::Px(PANEL_WIDTH),
            height: Val::Px(PANEL_HEIGHT),
            padding: UiRect::all(Val::Px(8.0)),
            column_gap: Val::Px(10.0),
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(PANEL_BG),
        BorderColor::all(color),
        children![
            (
                Name::new("PlayerPortrait"),
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
            slot_content(font, slot),
        ],
    )
}

/// 敌人面板那一列（右下角，**从下往上长**）。
///
/// 行**不是**逐行绝对定位的：那样每加一行内容就要改一次行高常量，忘了就重叠。
/// 交给 flex 之后"几行、多高"由内容自己决定——多一行字数也不会撞上。
pub fn enemy_column() -> impl Bundle {
    (
        Name::new("EnemyPanels"),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(14.0),
            // 列从底边长上去：`column_reverse` 让**第一个**子节点贴底
            bottom: Val::Px(14.0),
            flex_direction: FlexDirection::ColumnReverse,
            row_gap: Val::Px(ENEMY_ROW_GAP),
            ..default()
        },
    )
}

/// **一个敌人的一行**：只有内容块（没有头像——N 行时头像会把面板撑得过高，
/// 而行首的名字已经能分清是谁）。
///
/// `index` 是**名次**（0 = 离玩家最近）：行池按下标建好，每帧只改内容与显隐，
/// 所以敌人数量变化不会增删实体（与时间轴色块池同一个做法）。
pub fn enemy_row(font: &Handle<Font>, index: usize) -> impl Bundle {
    let slot = PanelSlot::Enemy(index);
    let color = faction_color(Faction::Enemy);
    (
        Name::new(format!("Enemy{}Row", index + 1)),
        UnitPanel { slot },
        Node {
            width: Val::Px(PANEL_WIDTH),
            // **给它一个下限**：五行内容（状态行 / 洞察力 / HP / EN / 行动行）
            // 在自动高度下会被压到 52px——文字本身不参与高度计算，于是行会挤在一起。
            // 这个数是按玩家面板同样的内容量定的（那边固定 104px，这里去掉头像那一侧）
            min_height: Val::Px(ENEMY_ROW_MIN_HEIGHT),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
            flex_direction: FlexDirection::Column,
            border_radius: BorderRadius::all(Val::Px(10.0)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(PANEL_BG),
        BorderColor::all(color),
        children![slot_content(font, slot)],
    )
}

/// 相邻两行之间的间隙（像素）。
pub const ENEMY_ROW_GAP: f32 = 6.0;
/// 一行敌人的**最小高度**（像素）。
///
/// 内容量：状态行 / 洞察力读数 / HP 条 / EN 条 / 行动行 = 五行。
/// 自动高度量不准文字（`ComputedNode` 里文本节点报 0），所以给一个下限兜住。
pub const ENEMY_ROW_MIN_HEIGHT: f32 = 96.0;
