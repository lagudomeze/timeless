//! 单位状态面板：头像 + HP / EN 条 + 状态行（左下 = 玩家，右下 = 敌人）。
//!
//! 「滑动条」是**只读进度条**：条长 = 数值比例，HUD 不改任何游戏状态。

use bevy::prelude::*;

use crate::ai::Intent;
use crate::combat::defense::{Dodging, Parrying, Stamina};
use crate::combat::{Faction, Health};
use crate::movement::{Cell, Jumping};
use crate::timeline::Ready;

use super::actions::ActionLabel;
use super::{
    EN_COLOR, HP_COLOR, HudCache, PANEL_BG, TRACK_BG, faction_color, hud_text, hud_text_tinted,
};

/// 面板整体尺寸（像素，还会被 `UiScale` 缩放）。
pub const PANEL_WIDTH: f32 = 340.0;
/// 见 [`PANEL_WIDTH`]。
pub const PANEL_HEIGHT: f32 = 104.0;
/// 头像边长。
pub const PORTRAIT_SIZE: f32 = 64.0;

/// 数值 → 0..=1 的比例（`max <= 0` 视为空）。
pub fn bar_fraction(current: f32, max: f32) -> f32 {
    if max <= 0.0 {
        0.0
    } else {
        (current / max).clamp(0.0, 1.0)
    }
}

/// 数值 → 条宽（`Val::Percent`）。
pub fn bar_percent(current: f32, max: f32) -> Val {
    Val::Percent(bar_fraction(current, max) * 100.0)
}

/// 阵营 → 实体名前缀（`PlayerPanel` / `EnemyPanel`）。
fn faction_prefix(faction: Faction) -> &'static str {
    match faction {
        Faction::Player => "Player",
        Faction::Enemy => "Enemy",
    }
}

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

/// 一帧的单位状态（面板文本全部由它推导）。
#[derive(Debug, Clone, Copy, PartialEq)]
struct UnitRow {
    faction: Faction,
    health: Health,
    stamina: Option<Stamina>,
    cell: Cell,
    position: Vec3,
    ready: bool,
    dodging: bool,
    parrying: bool,
    airborne: bool,
    intent: Option<Intent>,
}

/// 面板快照缓存：与上一帧完全相同就整帧不碰 UI。
#[derive(Debug, Default, Clone, PartialEq)]
pub struct UnitPanelCache {
    rows: [Option<UnitRow>; 2],
}

/// 阵营 → 快照下标（玩家在前）。
fn slot(faction: Faction) -> usize {
    match faction {
        Faction::Player => 0,
        Faction::Enemy => 1,
    }
}

impl UnitRow {
    /// 状态行：`PLAYER · ready · cell (1,1)`。
    fn state_line(&self, to_player: Option<f32>) -> String {
        let name = match self.faction {
            Faction::Player => "PLAYER",
            Faction::Enemy => "ENEMY",
        };
        // 跳跃是「谁都别想插队」的状态，值得单独标出来
        let defense = if self.airborne {
            format!("{} · air", self.defense_label())
        } else {
            self.defense_label().to_string()
        };
        let mut line = format!(
            "{name} · {defense} · cell ({:>2},{:>2})",
            self.cell.x, self.cell.z
        );
        if let Some(distance) = to_player {
            line.push_str(&format!(" · dist {distance:.1}"));
        }
        if let Some(intent) = self.intent {
            line.push_str(&format!(" · {}", intent_label(intent)));
        }
        line
    }

    /// 防御 / 就绪状态：防御标记优先。
    fn defense_label(&self) -> &'static str {
        if self.dodging {
            "dodging"
        } else if self.parrying {
            "parrying"
        } else if self.ready {
            "ready"
        } else {
            "busy"
        }
    }
}

/// 敌人意图的可读标签。
pub fn intent_label(intent: Intent) -> &'static str {
    match intent {
        Intent::Idle => "idle",
        Intent::Approach => "approach",
        Intent::Melee => "melee",
        Intent::Shoot => "shoot",
        Intent::Retreat => "retreat",
        Intent::Dodge => "dodge",
    }
}

/// 把 HP / EN / 状态行写进面板。
/// 单位快照查询（实体 + 阵营 + 血量 + 格 + 位姿 + 精力）。
type UnitQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Faction,
        &'static Health,
        &'static Cell,
        &'static Transform,
        Option<&'static Stamina>,
    ),
>;

/// 把 HP / EN / 状态行写进面板。
#[allow(clippy::too_many_arguments)]
pub fn update_unit_panels_system(
    units: UnitQuery<'_, '_>,
    mut cache: ResMut<HudCache>,
    mut bars: Query<(&PanelBar, &mut Node)>,
    mut texts: Query<(&PanelText, &mut Text)>,
    ready: Query<(), With<Ready>>,
    dodging: Query<(), With<Dodging>>,
    parrying: Query<(), With<Parrying>>,
    airborne: Query<(), With<Jumping>>,
    intents: Query<&Intent>,
) {
    let rows: Vec<UnitRow> = units
        .iter()
        .map(
            |(entity, faction, health, cell, transform, stamina)| UnitRow {
                faction: *faction,
                health: *health,
                stamina: stamina.copied(),
                cell: *cell,
                position: transform.translation,
                ready: ready.get(entity).is_ok(),
                dodging: dodging.get(entity).is_ok(),
                parrying: parrying.get(entity).is_ok(),
                airborne: airborne.get(entity).is_ok(),
                intent: intents.get(entity).ok().copied(),
            },
        )
        .collect();

    // 快照比对：这一帧与上一帧一模一样，就一个 UI 组件都不碰
    let mut snapshot: [Option<UnitRow>; 2] = [None, None];
    for row in &rows {
        snapshot[slot(row.faction)] = Some(*row);
    }
    if cache.units.rows == snapshot {
        return;
    }
    cache.units.rows = snapshot;

    let player_position = rows
        .iter()
        .find(|row| row.faction == Faction::Player)
        .map(|row| row.position);
    let row_of = |faction: Faction| rows.iter().find(|row| row.faction == faction);

    for (bar, mut node) in &mut bars {
        let percent = match bar {
            PanelBar::Hp(faction) => row_of(*faction)
                .map(|row| bar_percent(row.health.current, row.health.max))
                .unwrap_or(Val::Percent(0.0)),
            PanelBar::En(faction) => row_of(*faction)
                .and_then(|row| row.stamina)
                .map(|stamina| bar_percent(stamina.current as f32, stamina.max as f32))
                .unwrap_or(Val::Percent(0.0)),
        };
        node.width = percent;
    }

    for (label, mut text) in &mut texts {
        let new = match label {
            PanelText::Hp(faction) => row_of(*faction)
                .map(|row| format!("HP {:.0} / {:.0}", row.health.current, row.health.max)),
            PanelText::En(faction) => row_of(*faction).map(|row| match row.stamina {
                Some(stamina) => format!("EN {} / {}", stamina.current, stamina.max),
                None => "EN -".to_string(),
            }),
            PanelText::State(faction) => row_of(*faction).map(|row| {
                let distance = (row.faction == Faction::Enemy)
                    .then(|| player_position.map(|player| row.position.distance(player)))
                    .flatten();
                row.state_line(distance)
            }),
        };
        **text = new.unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_percent_maps_the_ratio_and_clamps() {
        assert_eq!(bar_percent(50.0, 100.0), Val::Percent(50.0));
        assert_eq!(bar_percent(5.0, 5.0), Val::Percent(100.0));
        assert_eq!(bar_percent(-3.0, 10.0), Val::Percent(0.0), "负值夹到 0");
        assert_eq!(bar_percent(30.0, 10.0), Val::Percent(100.0), "溢出夹到 100");
        assert_eq!(bar_percent(1.0, 0.0), Val::Percent(0.0), "max = 0 视为空");
    }

    #[test]
    fn state_line_carries_defense_and_intent() {
        let row = UnitRow {
            faction: Faction::Enemy,
            health: Health::new(50.0),
            stamina: None,
            cell: Cell::new(3, 3),
            position: Vec3::ZERO,
            ready: false,
            dodging: true,
            parrying: false,
            airborne: false,
            intent: Some(Intent::Approach),
        };
        let line = row.state_line(Some(7.12));
        assert!(line.contains("ENEMY"), "{line}");
        assert!(
            line.contains("dodging"),
            "防御标记优先于 ready/busy：{line}"
        );
        assert!(line.contains("cell ( 3, 3)"), "{line}");
        assert!(line.contains("dist 7.1"), "{line}");
        assert!(line.contains("approach"), "{line}");
    }

    /// 数据没变时面板整帧不写；数值一变就必须跟着变。
    #[test]
    fn panels_are_left_alone_when_nothing_changes() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<HudCache>()
            .add_systems(Update, update_unit_panels_system);
        let player = app
            .world_mut()
            .spawn((
                Faction::Player,
                Health::new(50.0),
                Cell::new(1, 1),
                Transform::from_xyz(3.0, 0.0, 3.0),
                Stamina::new(5),
            ))
            .id();
        let hp_text = app
            .world_mut()
            .spawn((PanelText::Hp(Faction::Player), Text::new("")))
            .id();
        let hp_bar = app
            .world_mut()
            .spawn((PanelBar::Hp(Faction::Player), Node::default()))
            .id();

        app.update();
        assert_eq!(app.world().get::<Text>(hp_text).unwrap().0, "HP 50 / 50");
        assert_eq!(
            app.world().get::<Node>(hp_bar).unwrap().width,
            Val::Percent(100.0)
        );

        // 哨兵值：数据没变时系统不该把它盖回去
        app.world_mut().get_mut::<Text>(hp_text).unwrap().0 = "SENTINEL".to_string();
        app.update();
        assert_eq!(
            app.world().get::<Text>(hp_text).unwrap().0,
            "SENTINEL",
            "数据没变就不该重写 UI"
        );

        // 掉一半血：文本与条宽都要跟上
        app.world_mut().get_mut::<Health>(player).unwrap().current = 25.0;
        app.update();
        assert_eq!(app.world().get::<Text>(hp_text).unwrap().0, "HP 25 / 50");
        assert_eq!(
            app.world().get::<Node>(hp_bar).unwrap().width,
            Val::Percent(50.0)
        );
    }
}
