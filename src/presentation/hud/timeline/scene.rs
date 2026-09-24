//! 时间轴的**场景**：开局的 UI 夹具（车道 / 色块池 / 刻度 / 候场区）+ 节点标记组件。
//!
//! 这里只**建**实体，不读游戏状态、不做任何更新——每帧的重排归 [`super::system`]。
//! 建完就不增删实体（池子的意思）：之后只改 `Node` / `Text` / `BackgroundColor`。
//!
//! 结构：状态行 → `[行首字母列 | 车道列 | 候场区]`。
//! 刻度与"现在"刻线**横跨所有车道**，所以它们挂在车道容器上（绝对定位 + 高度 100%）。
//! 行首字母列、车道列、候场区都用同样的「[`LANE_HEIGHT`] + [`LANE_GAP`]」节奏，因此
//! 三条竖列天然对齐：第 N 行的方块 / 色块 / 候场块永远在同一水平线上。

use bevy::prelude::*;

use super::super::hud_text_tinted;
use super::model::{
    BLOCK_POOL_PER_LANE, LANE_GAP, LANE_HEIGHT, LANE_POOL, STAGING_WIDTH, TICK_POOL, TICK_SECONDS,
    WINDOW_SECONDS,
};
use super::readout::{TimelineReadout, TimelineReadoutText};

/// 车道底色：比轨道更淡，它只是"行"，不抢色块。
const LANE_BG: Color = Color::srgba(1.0, 1.0, 1.0, 0.05);

/// 时间轴左侧的状态行（RUNNING / FROZEN + 冻结原因）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct TimelineStateLabel;

/// 色块（`slot` = 排序后的第几名）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct TimelineBlock {
    /// 第几条车道（= 哪个单位）
    pub lane: usize,
    /// 该车道内的第几个色块
    pub slot: usize,
}

/// 色块里的阵营字母。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct TimelineBlockLabel {
    pub lane: usize,
    pub slot: usize,
}

/// 色块里的**结算刻线**（`execute_at` 在块内的位置）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct TimelineBlockMark {
    pub lane: usize,
    pub slot: usize,
}

/// 一条车道（= 一个单位的一行）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct TimelineLane {
    pub index: usize,
}

/// 车道行首的归属字母（`P` / `E`），颜色 = 阵营色。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct TimelineLaneLabel {
    pub index: usize,
}

/// 候场区的小方块：这个单位现在能决策、但还没声明。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct TimelineReadyChip {
    pub index: usize,
}

/// 候场方块里的归属字母。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct TimelineReadyLabel {
    pub index: usize,
}

/// 生成时间轴（返回根实体，由 [`super::super::layout`] 挂到 HUD 根上）。
pub fn spawn_timeline(commands: &mut Commands, font: &Handle<Font>) -> Entity {
    let root = commands
        .spawn((
            Name::new("Timeline"),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                left: Val::Percent(15.0),
                width: Val::Percent(70.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
            children![
                (
                    Name::new("TimelineState"),
                    hud_text_tinted(font, 12.0, "", Color::srgb(0.80, 0.86, 0.95)),
                    TimelineStateLabel,
                ),
                // 悬停读数：滑到色块上看"谁 · 什么 · 打哪 · 还剩多久 · 能不能打断"。
                // 默认隐藏，鼠标离开时间轴就收起。
                (
                    Name::new("TimelineReadout"),
                    Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        display: Display::None,
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.92)),
                    TimelineReadout,
                    children![(
                        Name::new("TimelineReadoutText"),
                        hud_text_tinted(font, 11.0, "", Color::srgb(0.86, 0.92, 1.0)),
                        TimelineReadoutText,
                    )],
                ),
            ],
        ))
        .id();

    // 行首字母列：与车道一一对应
    let label_column = commands
        .spawn((
            Name::new("TimelineLaneLabels"),
            Node {
                width: Val::Px(10.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(LANE_GAP),
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .id();

    let lanes = commands
        .spawn((
            Name::new("TimelineLanes"),
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(LANE_GAP),
                ..default()
            },
        ))
        .id();

    for lane in 0..LANE_POOL {
        // 行首字母
        let label = commands
            .spawn((
                Name::new(format!("TimelineLaneLabel{lane}")),
                TimelineLaneLabel { index: lane },
                Node {
                    height: Val::Px(LANE_HEIGHT),
                    align_items: AlignItems::Center,
                    ..default()
                },
                hud_text_tinted(font, 9.0, "", Color::WHITE),
            ))
            .id();
        commands.entity(label_column).add_child(label);

        // 车道本体 + 它自己的色块池
        let row = commands
            .spawn((
                Name::new(format!("TimelineLane{lane}")),
                TimelineLane { index: lane },
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(LANE_HEIGHT),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(LANE_BG),
            ))
            .id();
        let blocks: Vec<Entity> = (0..BLOCK_POOL_PER_LANE)
            .map(|slot| spawn_lane_block(commands, font, lane, slot))
            .collect();
        commands.entity(row).add_children(&blocks);
        commands.entity(lanes).add_child(row);
    }

    // 秒刻度：常驻、不随事件增删——时间轴的"尺子"，横跨所有车道
    let ticks: Vec<Entity> = (1..TICK_POOL)
        .map(|index| {
            let left = index as f32 * TICK_SECONDS / WINDOW_SECONDS * 100.0;
            commands
                .spawn((
                    Name::new(format!("TimelineTick{index}")),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(left),
                        width: Val::Px(1.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.14)),
                ))
                .id()
        })
        .collect();
    commands.entity(lanes).add_children(&ticks);

    // 「现在」刻线：色块的最左端就是它。`ZIndex(1)` 让它压在色块之上，
    // 否则刚起步的行动色块（左边界 0%）会把这条参照线吃掉。
    let playhead = commands
        .spawn((
            Name::new("TimelinePlayhead"),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                width: Val::Px(2.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.55)),
            ZIndex(1),
        ))
        .id();
    commands.entity(lanes).add_child(playhead);

    // 候场区：只显示"能决策、但还没声明"的人
    let staging = commands
        .spawn((
            Name::new("TimelineStaging"),
            Node {
                width: Val::Px(STAGING_WIDTH),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(LANE_GAP),
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .id();
    let chips: Vec<Entity> = (0..LANE_POOL)
        .map(|index| {
            commands
                .spawn((
                    Name::new(format!("TimelineReady{index}")),
                    TimelineReadyChip { index },
                    Node {
                        width: Val::Px(LANE_HEIGHT),
                        height: Val::Px(LANE_HEIGHT),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border_radius: BorderRadius::all(Val::Px(2.0)),
                        display: Display::None,
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    children![(
                        Name::new(format!("TimelineReadyLabel{index}")),
                        TimelineReadyLabel { index },
                        hud_text_tinted(font, 9.0, "", Color::srgb(0.05, 0.06, 0.09)),
                    )],
                ))
                .id()
        })
        .collect();
    commands.entity(staging).add_children(&chips);

    let rows = commands
        .spawn((
            Name::new("TimelineRows"),
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                ..default()
            },
        ))
        .id();
    commands
        .entity(rows)
        .add_children(&[label_column, lanes, staging]);
    commands.entity(root).add_child(rows);
    root
}

/// 一条车道内的一个色块：底色 + 阵营字母 + 结算刻线，默认隐藏。
///
/// `slot` 顺序 = Children 顺序 = z 序（后面的画在上面）。一个单位同一时刻只会有一个
/// 未落地的行动（决策槽被占住的那段时间），池子留 2 个只是为了给"同一帧内换手"留余量。
fn spawn_lane_block(
    commands: &mut Commands,
    font: &Handle<Font>,
    lane: usize,
    slot: usize,
) -> Entity {
    commands
        .spawn((
            Name::new(format!("TimelineBlockLane{lane}Slot{slot}")),
            TimelineBlock { lane, slot },
            // 悬停识别：色块挂 `Interaction` 才能被鼠标读到（读数与高亮的入口）
            Interaction::default(),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(0.0),
                width: Val::Percent(0.0),
                height: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(0.0)),
                align_items: AlignItems::Center,
                display: Display::None,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            children![
                (
                    Name::new(format!("TimelineBlockLabelLane{lane}Slot{slot}")),
                    hud_text_tinted(font, 9.0, "", Color::srgb(0.06, 0.07, 0.10)),
                    TimelineBlockLabel { lane, slot },
                ),
                (
                    Name::new(format!("TimelineBlockMarkLane{lane}Slot{slot}")),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(50.0),
                        width: Val::Px(2.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
                    TimelineBlockMark { lane, slot },
                ),
            ],
        ))
        .id()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::hud::timeline::model::{BLOCK_POOL_PER_LANE, LANE_POOL, TICK_POOL};

    /// 车道容器里的顺序：车道（0 → N）→ 秒刻度 → "现在"刻线。
    ///
    /// 顺序 = z 序（后面的画在上面）：色块压在刻度上，而"现在"刻线抬到最上层；
    /// 每条车道又各自拥有自己的色块池（slot 0 起步）。
    #[test]
    fn every_lane_owns_its_own_block_pool() {
        let mut world = World::new();
        let font = Handle::<Font>::default();
        let root = spawn_timeline(&mut world.commands(), &font);
        world.flush();

        let mut query = world.query::<(Entity, &Name)>();
        let named: Vec<(Entity, String)> = query
            .iter(&world)
            .map(|(entity, name)| (entity, name.as_str().to_string()))
            .collect();
        let find = |target: &str| {
            named
                .iter()
                .find(|(_, name)| name == target)
                .map(|(entity, _)| *entity)
                .unwrap_or_else(|| panic!("找不到名为 {target} 的实体"))
        };
        let name_of = |entity: Entity| {
            named
                .iter()
                .find(|(candidate, _)| *candidate == entity)
                .map(|(_, name)| name.clone())
                .expect("HUD 节点都应当有名字")
        };

        assert_eq!(name_of(root), "Timeline");
        let lanes = find("TimelineLanes");
        let children: Vec<String> = world
            .get::<Children>(lanes)
            .expect("车道容器应当有子节点")
            .iter()
            .map(name_of)
            .collect();

        let expected: Vec<String> = (0..LANE_POOL)
            .map(|lane| format!("TimelineLane{lane}"))
            .chain((1..TICK_POOL).map(|index| format!("TimelineTick{index}")))
            .chain(std::iter::once("TimelinePlayhead".to_string()))
            .collect();
        assert_eq!(children, expected, "车道在前、跨行刻度与刻线在后");

        for lane in 0..LANE_POOL {
            let row = find(&format!("TimelineLane{lane}"));
            let blocks: Vec<String> = world
                .get::<Children>(row)
                .expect("车道应当有色块池")
                .iter()
                .map(name_of)
                .collect();
            let expected: Vec<String> = (0..BLOCK_POOL_PER_LANE)
                .map(|slot| format!("TimelineBlockLane{lane}Slot{slot}"))
                .collect();
            assert_eq!(blocks, expected, "车道 {lane} 的色块池必须是 slot 0 起步");
        }
    }
}
