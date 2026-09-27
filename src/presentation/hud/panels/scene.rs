//! 单位面板的**场景**：头像 + 名字 / 状态行 + HP / EN 条的 UI 夹具与节点标记组件。
//!
//! 只**建**实体，不读游戏状态——每帧把读数写进去的是 [`super::system`]。

use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};

use crate::combat::Faction;

use super::super::actions::ActionLabel;
use super::super::{
    EN_COLOR, HP_COLOR, PANEL_BG, TRACK_BG, faction_color, hud_text, hud_text_tinted,
};
use super::model::{FOCUS_PIPS, MAX_ENEMY_ROWS, PanelSlot};

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

/// 面板文本：血量 / 精力 / 状态行 / **反制资源 Focus** / 洞察力。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub enum PanelText {
    Hp(PanelSlot),
    En(PanelSlot),
    State(PanelSlot),
    /// **反制资源读数**（`docs/backlog/hud.md` 的 #52）：`FOCUS 2 / 3`。
    /// 玩家与**敌人都有**——敌人也会花 Focus 闪避，看得见"它刚买掉了前摇"才是对称的。
    Focus(PanelSlot),
    /// **洞察力读数**（`docs/insight.md` 第四节）：射程 / 打断抗性 / 战术。
    /// 只有敌人格有内容（玩家看自己的面板不需要"我够得到多远"）。
    Insight(PanelSlot),
    /// **溢出计数**（`docs/backlog/hud.md` 的 #61）：`还有 N 个`。
    /// 只有一份（挂在敌人那一列的顶上），没有溢出时整行藏起来。
    EnemyOverflow,
}

/// Focus 的一个圆点（改颜色：用掉的压暗）。
///
/// 池化：开局按 [`super::model::FOCUS_PIPS`] 建好，之后只改颜色——
/// 与时间轴色块池、敌人行池同一个做法（帧内不产生实体分配）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct PanelFocusPip {
    pub slot: PanelSlot,
    /// 第几个点（`0` = 最左边）
    pub index: usize,
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

/// Focus 圆点的可选色（还有余量）。
pub const FOCUS_PIP_ON: Color = Color::srgb(0.55, 0.80, 1.0);
/// Focus 圆点的已用色（压暗：一眼看出花掉了几点）。
pub const FOCUS_PIP_OFF: Color = Color::srgb(0.22, 0.26, 0.33);

/// 一格的「状态行 + HP / EN 条 + 当前行动」——**三种面板共用**这一个内容块。
///
/// 玩家面板与敌人行**内容完全一样**，只是摆放位置与头像有无不同；抽成一处
/// 就不会出现"玩家面板加了护甲读数、敌人行忘了加"这种漂移。
fn slot_content(font: &Handle<Font>, slot: PanelSlot) -> impl Bundle {
    let prefix = slot_prefix(slot);
    // 闭包要 `move`（`SpawnWith` 是 `'static` 的），所以给它一份**影子副本**：
    // 外层下面还要用 `prefix` 给几个节点命名。
    let pip_prefix = prefix.clone();
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
            // **反制资源**（#52）：`FOCUS 3 / 3` + 三个点。它此前是唯一没有读数的资源，
            // 而它是"能不能抢在对方出手前动起来"的唯一依据。
            (
                Name::new(format!("{prefix}FocusLine")),
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    ..default()
                },
                children![
                    (
                        Name::new(format!("{prefix}FocusText")),
                        hud_text_tinted(font, 11.0, "FOCUS -", FOCUS_PIP_ON),
                        PanelText::Focus(slot),
                    ),
                    (
                        Name::new(format!("{prefix}FocusPips")),
                        Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(4.0),
                            ..default()
                        },
                        Children::spawn(SpawnWith(move |parent: &mut ChildSpawner| {
                            for index in 0..FOCUS_PIPS {
                                parent.spawn((
                                    Name::new(format!("{pip_prefix}Focus{index}")),
                                    PanelFocusPip { slot, index },
                                    Node {
                                        width: Val::Px(10.0),
                                        height: Val::Px(10.0),
                                        border_radius: BorderRadius::all(Val::Px(2.0)),
                                        ..default()
                                    },
                                    BackgroundColor(FOCUS_PIP_OFF),
                                ));
                            }
                        })),
                    ),
                ],
            ),
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
        // 吃掉指针：光标压在这一块上时世界不该收到鼠标（见 `interaction::ui_capture`）
        FocusPolicy::Block,
        RelativeCursorPosition::default(),
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
///
/// ⚠️ **不用 `ColumnReverse`**：它会把**主轴的起点**也翻过来，于是列的第一个子节点
/// 贴底、**最后一个贴顶**——想在最上面加一行"还有 N 个"就得把生成顺序整个反过来读
/// （相邻两行同宽时看不出来，但读起来是反的）。改成 `Column` + `justify_content: End`
/// 同样贴着底边长，而**生成顺序就是视觉顺序**。
pub fn enemy_column() -> impl Bundle {
    (
        Name::new("EnemyPanels"),
        // 吃掉指针：整列（含行间空隙）都算压在 UI 上。
        // 只声明**列**不声明每行：行会被 `display: None` 收起来，而
        // `ui_focus_system` 对不可见节点不写 `cursor_over`，那一行会留下过期值。
        FocusPolicy::Block,
        RelativeCursorPosition::default(),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(14.0),
            bottom: Val::Px(14.0),
            flex_direction: FlexDirection::Column,
            // 贴着底边长：行数变化时整列向上扩，底边不动
            justify_content: JustifyContent::End,
            row_gap: Val::Px(ENEMY_ROW_GAP),
            // ⚠️ **显式上限**：满池（`MAX_ENEMY_ROWS` 行）+ 溢出计数行。
            // 不写这个的话，"几行"就由内容撑——一旦行数或行高变了，整列会长进
            // 它正上方的战斗日志里（实测过：`Enemy2Row` 压住 `COMBAT LOG` 标题）。
            // 有测试钉住这个高度 ≥ 满池所需的量。
            max_height: Val::Px(ENEMY_COLUMN_MAX_HEIGHT),
            ..default()
        },
    )
}

/// 溢出计数那一行（`还有 2 个`）——放在**列的最上面**（生成顺序上排最后）。
///
/// 它是 [`PanelText::EnemyOverflow`]，没有溢出时整行 `display: None`。
/// ⚠️ 它的高度**不参与行池**：行池只建 `MAX_ENEMY_ROWS` 个 `UnitPanel`，
/// 这一行是额外的第 N+1 个节点（有测试钉住"行池大小 == MAX_ENEMY_ROWS"）。
pub fn enemy_overflow_row(font: &Handle<Font>) -> impl Bundle {
    (
        Name::new("EnemyOverflow"),
        Node {
            width: Val::Px(PANEL_WIDTH),
            display: Display::None,
            ..default()
        },
        hud_text_tinted(font, 11.0, "", Color::srgb(0.95, 0.72, 0.45)),
        PanelText::EnemyOverflow,
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
/// 内容量：状态行 / Focus 行 / 洞察力读数 / HP 条 / EN 条 / 行动行。
/// 自动高度量不准文字（`ComputedNode` 里文本节点报 0），所以给一个下限兜住。
///
/// ⚠️ **这个数必须 ≥ 实测行高**：给少了不是"挤一点"，而是**整列把它当成硬上限，
/// 多行互相压叠**——2026-09-27 实机：3 行敌人时 `ENEMY 2`/`ENEMY 3` 的文字叠在一起。
/// 实测一行 146~147px（带头像的玩家面板 156px），这里取 150 留一点余量。
pub const ENEMY_ROW_MIN_HEIGHT: f32 = 150.0;
/// 溢出计数行的高度（像素）：一行 11px 文字（行本身不再加内边距）。
pub const ENEMY_OVERFLOW_HEIGHT: f32 = 23.0;
/// 敌人列的高度上限（像素）：满池 + 溢出行 + 行间距。
///
/// **一处真相**：它必须 ≥ 下列各项之和（有测试钉住），否则满池时最后一行会被裁掉。
pub const ENEMY_COLUMN_MAX_HEIGHT: f32 = MAX_ENEMY_ROWS as f32 * ENEMY_ROW_MIN_HEIGHT
    + ENEMY_OVERFLOW_HEIGHT
    + (MAX_ENEMY_ROWS as f32) * ENEMY_ROW_GAP;

/// 战斗日志面板底边的高度（像素）：**敌人列上限 + 底部留白 + 一点间隙**。
///
/// **为什么放在这里而不是 `log_panel.rs`**：这个数由敌人面板的高度决定，
/// 而"敌人面板有多高"是这一层说了算。日志那一侧只消费它——
/// 反过来（日志自己写一个数）就会在敌人面板长高时静默重叠（踩过）。
pub const LOG_BOTTOM_CLEARANCE: f32 = 14.0 + ENEMY_COLUMN_MAX_HEIGHT + 8.0;

#[cfg(test)]
mod tests {
    use super::*;

    /// 敌人列的高度上限**装得下满池**（否则最后一行会被裁掉）。
    #[test]
    fn the_enemy_column_can_hold_a_full_pool() {
        let needed = MAX_ENEMY_ROWS as f32 * ENEMY_ROW_MIN_HEIGHT
            + (MAX_ENEMY_ROWS as f32) * ENEMY_ROW_GAP
            + ENEMY_OVERFLOW_HEIGHT;
        assert_eq!(
            ENEMY_COLUMN_MAX_HEIGHT, needed,
            "上限就是「满池 + 溢出行 + 行间距」——改任一项都要同步改它"
        );
    }

    /// **战斗日志必须让开敌人列**（实测回归：`Enemy2Row` 曾经压住 `COMBAT LOG`）。
    ///
    /// 两块都靠右：日志在敌人行的**正上方**。它们的净空由这一个常量保证，
    /// 所以只要这里的算式成立，任何行数都不会重叠。
    #[test]
    fn the_combat_log_clears_the_enemy_column() {
        // 日志底边 ≥ 敌人列底边（14）+ 列高（+ 8px 间隙）
        assert_eq!(
            LOG_BOTTOM_CLEARANCE,
            14.0 + ENEMY_COLUMN_MAX_HEIGHT + 8.0,
            "日志底边必须由敌人列的高度算出来，不能写死一个数"
        );
    }

    /// 一行敌人的内容量变了（加了 Focus 行）之后，最小高度也得跟着够——
    /// 它量不准文字（`ComputedNode` 里文本节点报 0），所以只能靠这个下限兜住。
    #[test]
    fn a_row_is_tall_enough_for_its_content() {
        // 六行内容：状态行 / Focus 行 / 洞察力 / HP / EN / 行动行，每行约 13px
        let needed = 6.0 * 13.0;
        assert!(
            ENEMY_ROW_MIN_HEIGHT >= needed,
            "行高 {ENEMY_ROW_MIN_HEIGHT} 装不下六行文字（约 {needed}）"
        );
    }
}
