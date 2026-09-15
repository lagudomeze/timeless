//! 顶部时间轴：把「谁在什么时候出手」画成一排色块。
//!
//! 横轴是**虚拟秒**：左端 = 现在，右端 = `WINDOW_SECONDS` 之后；
//! 每 `TICK_SECONDS` 一条淡刻度，静止时也能读出"多长"。
//!
//! **色块 = 占用，刻线 = 结算**（两种信息走两条视觉通道）：
//!
//! - 色块左边界 = **声明时刻**（`declared_at`），宽度 = 前摇 + 后摇
//!   （见 [`crate::timeline::timing`]）——所以它天然从"现在"刻线长出去，
//!   回答"我按下去之后要忙多久、什么时候能再决策"；
//! - 块内那条白色竖线 = **结算时刻**（`execute_at` = 声明时刻 + 前摇），
//!   回答"我这一手什么时候真的发生 / 什么时候会挨打"。
//!
//! 颜色 = 阵营（蓝 = 玩家、红 = 敌人），**前摇中**（还撤得掉）的行动用半透明表示。
//!
//! **每个单位一行（lane）**：一条横轴解决不了"谁在动手"——两个单位同时出手时色块会
//! 叠在一起。所以纵轴按单位分道：玩家在最上面一行，其余按稳定顺序往下排，行首的字母
//! 就是归属。没有排期的人不占横轴，而是站在**右侧候场区**（`TimelineStaging`）：
//! 那里只显示"现在能决策、但还没声明"的人，一眼能看出还剩谁没动。
//!
//! 无回合模型里没有「回合格子」，所以时间轴**不是**回合队列，而是一段连续时间窗；
//! 玩家等输入时虚拟时间冻结，色块也跟着停住（这正是玩家读盘的时机）。

use bevy::prelude::*;

use crate::combat::Faction;
use crate::timeline::{DecisionSlot, PauseReasons, ScheduledAction};

use super::{HudCache, faction_color_alpha, hud_text_tinted};

/// 一个色块的「时间轴坐标 + 归属」（纯数据，用来比对是否需要重画）。
#[derive(Debug, Clone, Copy, PartialEq)]
struct TimelineSlot {
    left: f32,
    width: f32,
    /// 结算刻线在块内的位置（%）：前摇 / 总时长
    mark: f32,
    faction: Faction,
    draft: bool,
}

/// 时间轴快照缓存：与上一帧完全相同就整帧不碰 UI。
///
/// 冻结时这条路径收益最大——玩家等输入时虚拟时间冻结，`now` 不变、队列不变，
/// 时间轴于是完全静止，每帧的 `Node` 写入全部省掉。
#[derive(Debug, Default, Clone, PartialEq)]
pub struct TimelineCache {
    state: String,
    /// 每条车道的色块（下标 = lane）
    lanes: Vec<Vec<TimelineSlot>>,
    /// 站在候场区的车道（已就绪、还没声明）
    ready: Vec<usize>,
}

/// 时间轴视野（虚拟秒）：只画这段时间里会发生的行动。
pub const WINDOW_SECONDS: f32 = 4.0;
/// 刻度间隔（虚拟秒）。条子在冻结时是不动的，刻度就是它唯一的"尺子"。
pub const TICK_SECONDS: f32 = 0.5;
/// 刻度数量（`0` 处是"现在"刻线，不重复画）。
pub const TICK_POOL: usize = (WINDOW_SECONDS / TICK_SECONDS) as usize;
/// 色块最小宽度（百分比），免得极短动作看不见。
pub const MIN_BLOCK_WIDTH: f32 = 3.0;
/// 车道池大小（每个单位一行；常驻 1 玩家 + 1 敌人，留几个空位给召唤物）。
pub const LANE_POOL: usize = 4;
/// 单条车道的色块池：一个单位同一时刻只会有一个未落地的行动。
pub const BLOCK_POOL_PER_LANE: usize = 2;
/// 单条车道的高度（像素）。
pub const LANE_HEIGHT: f32 = 12.0;
/// 车道之间的竖直间隙（像素）。
pub const LANE_GAP: f32 = 2.0;
/// 右侧候场区宽度（像素）。
pub const STAGING_WIDTH: f32 = 24.0;
/// 车道底色：比轨道更淡，它只是"行"，不抢色块。
const LANE_BG: Color = Color::srgba(1.0, 1.0, 1.0, 0.05);

/// 时间轴左侧的状态行（WAITING / RUNNING + 草案提示）。
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

/// 阵营字母（色块 / 车道 / 候场方块共用）。
fn faction_letter(faction: Faction) -> &'static str {
    match faction {
        Faction::Player => "P",
        Faction::Enemy => "E",
    }
}

/// 车道排序权重：玩家永远在最上面一行，其余按阵营稳定排。
fn faction_rank(faction: Faction) -> u8 {
    match faction {
        Faction::Player => 0,
        Faction::Enemy => 1,
    }
}

/// 行动在时间轴上的（左边界 %, 宽度 %）；不在视野内返回 `None`。
///
/// `declared_at` 是**声明时刻**（不是执行时刻）：宽度 = 前摇 + 后摇 = 这一整段
/// 「该单位被占住」的时间，所以色块从"现在"长出去；结算点画在块内
/// [`resolve_mark_percent`] 的位置。
///
/// 已经开始的行动（`declared_at < now`，例如正在飞的火球）裁到左端而不是消失——
/// 玩家仍然看得见「它还在进行中」。
pub fn block_span(declared_at: f32, total: f32, now: f32, window: f32) -> Option<(f32, f32)> {
    if total <= 0.0 || window <= 0.0 {
        return None;
    }
    let start = declared_at - now;
    let end = start + total;
    if end <= 0.0 || start > window {
        return None;
    }
    let left = (start.max(0.0) / window) * 100.0;
    let right = (end.min(window) / window) * 100.0;
    Some((left, (right - left).max(MIN_BLOCK_WIDTH)))
}

/// 结算刻线在色块内的位置（%）：前摇占整段忙时间的比例。
///
/// `total = 前摇 + 后摇`，所以它永远落在块内（0% = 声明即结算，100% = 全在等结算）。
pub fn resolve_mark_percent(windup: f32, total: f32) -> f32 {
    if total <= 0.0 {
        return 0.0;
    }
    (windup / total * 100.0).clamp(0.0, 100.0)
}

/// 生成时间轴（返回根实体，由 [`super::layout`] 挂到 HUD 根上）。
///
/// 结构：状态行 → `[行首字母列 | 车道列 | 候场区]`。
/// 刻度与"现在"刻线**横跨所有车道**，所以它们挂在车道容器上（绝对定位 + 高度 100%）。
/// 行首字母列、车道列、候场区都用同样的「`LANE_HEIGHT` + `LANE_GAP`」节奏，因此
/// 三条竖列天然对齐：第 N 行的方块 / 色块 / 候场块永远在同一水平线上。
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
            children![(
                Name::new("TimelineState"),
                hud_text_tinted(font, 12.0, "", Color::srgb(0.80, 0.86, 0.95)),
                TimelineStateLabel,
            )],
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

/// 状态行文本（与下面几个文本查询两两互斥）。
type StateTextQuery<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<TimelineStateLabel>,
        Without<TimelineLaneLabel>,
        Without<TimelineBlockLabel>,
        Without<TimelineReadyLabel>,
    ),
>;

/// 车道行首字母。
type LaneLabelQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static TimelineLaneLabel,
        &'static mut Text,
        &'static mut TextColor,
    ),
    (
        Without<TimelineStateLabel>,
        Without<TimelineBlockLabel>,
        Without<TimelineReadyLabel>,
    ),
>;

/// 色块内的阵营字母。
type BlockLabelQuery<'w, 's> = Query<
    'w,
    's,
    (&'static TimelineBlockLabel, &'static mut Text),
    (
        Without<TimelineStateLabel>,
        Without<TimelineLaneLabel>,
        Without<TimelineReadyLabel>,
    ),
>;

/// 色块内的结算刻线。
type BlockMarkQuery<'w, 's> = Query<
    'w,
    's,
    (&'static TimelineBlockMark, &'static mut Node),
    (
        Without<TimelineBlock>,
        Without<TimelineReadyChip>,
        Without<TimelineLane>,
    ),
>;

/// 候场区方块。
type ReadyChipQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static TimelineReadyChip,
        &'static mut Node,
        &'static mut BackgroundColor,
    ),
    (Without<TimelineBlock>, Without<TimelineBlockMark>),
>;

/// 候场方块的字母。
type ReadyLabelQuery<'w, 's> = Query<
    'w,
    's,
    (&'static TimelineReadyLabel, &'static mut Text),
    (
        Without<TimelineStateLabel>,
        Without<TimelineLaneLabel>,
        Without<TimelineBlockLabel>,
    ),
>;

/// 每帧重排车道 / 色块 / 候场区 + 刷新状态行。
///
/// 查询之间用 `Without` 两两互斥（都碰 `Text` / `Node` / `BackgroundColor`），
/// 否则 Bevy 会报 B0001 参数冲突。
#[allow(clippy::too_many_arguments)]
pub fn update_timeline_system(
    reasons: Res<PauseReasons>,
    now: Res<Time<Virtual>>,
    actors: Query<(Entity, &Faction)>,
    slots: Query<&DecisionSlot>,
    actions: Query<&ScheduledAction>,
    mut cache: ResMut<HudCache>,
    mut states: StateTextQuery<'_, '_>,
    mut lane_labels: LaneLabelQuery<'_, '_>,
    mut blocks: Query<(&TimelineBlock, &mut Node, &mut BackgroundColor)>,
    mut marks: BlockMarkQuery<'_, '_>,
    mut labels: BlockLabelQuery<'_, '_>,
    mut chips: ReadyChipQuery<'_, '_>,
    mut chip_labels: ReadyLabelQuery<'_, '_>,
) {
    // 冻结的原因直接读出来：玩家一眼知道是"等我决策"、"我按了空格"还是"有人打过来"
    let state = if reasons.is_frozen() {
        format!("TIMELINE · FROZEN · {}", reasons.labels().join(" + "))
    } else {
        "TIMELINE · RUNNING".to_string()
    };

    // 1. 稳定的 actor → lane 映射：玩家排最上面，其余按实体序号（同帧可复现）
    let mut roster: Vec<(Entity, Faction)> = actors
        .iter()
        .map(|(entity, faction)| (entity, *faction))
        .collect();
    roster.sort_by_key(|(entity, faction)| (faction_rank(*faction), entity.index()));
    let lane_actor: Vec<Option<Entity>> = (0..LANE_POOL)
        .map(|lane| roster.get(lane).map(|(entity, _)| *entity))
        .collect();
    let lane_faction: Vec<Option<Faction>> = (0..LANE_POOL)
        .map(|lane| roster.get(lane).map(|(_, faction)| *faction))
        .collect();

    // 2. 每个单位只画自己那一行：行动色块 = 这段"被占住"的时间（从声明时刻长出去）
    let now_seconds = now.elapsed_secs();
    let mut lanes: Vec<Vec<TimelineSlot>> = vec![Vec::new(); LANE_POOL];
    for schedule in &actions {
        let Some(lane) = lane_actor
            .iter()
            .position(|actor| *actor == Some(schedule.actor))
        else {
            continue; // 行动者不在花名册里（刚销毁 / 还没组装）
        };
        let Some(faction) = lane_faction[lane] else {
            continue;
        };
        // 还没到点 = 前摇中 = 还能撤（半透明表示"这一手还改得动"）
        let draft = schedule.pending(now_seconds);
        let Some((left, width)) = block_span(
            schedule.declared_at,
            schedule.total(),
            now_seconds,
            WINDOW_SECONDS,
        ) else {
            continue;
        };
        lanes[lane].push(TimelineSlot {
            left,
            width,
            mark: resolve_mark_percent(schedule.windup(), schedule.total()),
            faction,
            draft,
        });
    }
    for lane in &mut lanes {
        // 同一行的先后顺序稳定：按左边界
        lane.sort_by(|a, b| a.left.total_cmp(&b.left));
    }

    // 3. 候场区：已就绪、还没有排期的人
    let ready_lanes: Vec<usize> = (0..LANE_POOL)
        .filter(|lane| {
            lane_actor[*lane].is_some_and(|actor| {
                slots.get(actor).copied().unwrap_or_default() == DecisionSlot::Empty
            }) && lanes[*lane].is_empty()
        })
        .collect();

    // 4. 快照比对：状态、车道内容、候场名单都没变就整帧不碰 UI
    if cache.timeline.state == state
        && cache.timeline.lanes == lanes
        && cache.timeline.ready == ready_lanes
    {
        return;
    }
    cache.timeline.state = state.to_string();
    cache.timeline.lanes.clone_from(&lanes);
    cache.timeline.ready.clone_from(&ready_lanes);

    // 5. 写 UI
    for mut text in &mut states {
        **text = state.to_string();
    }

    for (label, mut text, mut color) in &mut lane_labels {
        **text = lane_faction
            .get(label.index)
            .copied()
            .flatten()
            .map(faction_letter)
            .unwrap_or_default()
            .to_string();
        if let Some(faction) = lane_faction.get(label.index).copied().flatten() {
            *color = TextColor(faction_color_alpha(faction, 0.9));
        }
    }

    for (block, mut node, mut color) in &mut blocks {
        match lanes.get(block.lane).and_then(|lane| lane.get(block.slot)) {
            Some(slot) => {
                node.display = Display::Flex;
                node.left = Val::Percent(slot.left);
                node.width = Val::Percent(slot.width);
                *color = BackgroundColor(faction_color_alpha(
                    slot.faction,
                    if slot.draft { 0.5 } else { 1.0 },
                ));
            }
            None => {
                node.display = Display::None;
                // 顺手把几何归零：留着上一次的 left/width 就成了「幽灵色块」，
                // 以后若改成用 alpha 隐藏而不是 `Display::None`，它会直接显形
                node.left = Val::Percent(0.0);
                node.width = Val::Percent(0.0);
                *color = BackgroundColor(Color::NONE);
            }
        }
    }

    for (mark, mut node) in &mut marks {
        node.left = Val::Percent(
            lanes
                .get(mark.lane)
                .and_then(|lane| lane.get(mark.slot))
                .map(|slot| slot.mark)
                .unwrap_or_default(),
        );
    }

    for (label, mut text) in &mut labels {
        **text = lanes
            .get(label.lane)
            .and_then(|lane| lane.get(label.slot))
            .map(|slot| {
                let name = faction_letter(slot.faction);
                if slot.draft {
                    format!("{name}*")
                } else {
                    name.to_string()
                }
            })
            .unwrap_or_default();
    }

    for (chip, mut node, mut color) in &mut chips {
        let faction = ready_lanes
            .contains(&chip.index)
            .then(|| lane_faction.get(chip.index).copied().flatten())
            .flatten();
        match faction {
            Some(faction) => {
                node.display = Display::Flex;
                *color = BackgroundColor(faction_color_alpha(faction, 0.9));
            }
            None => {
                node.display = Display::None;
                *color = BackgroundColor(Color::NONE);
            }
        }
    }

    for (label, mut text) in &mut chip_labels {
        **text = lane_faction
            .get(label.index)
            .copied()
            .flatten()
            .map(faction_letter)
            .unwrap_or_default()
            .to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::timing;

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

    #[test]
    fn block_span_maps_seconds_to_percent_of_the_window() {
        // 现在 = 0，动作在 1.0s 处声明、总共占 1.0s，视野 4s：左边界 25%、宽度 25%
        let (left, width) = block_span(1.0, 1.0, 0.0, 4.0).unwrap();
        assert_eq!((left, width), (25.0, 25.0));
    }

    /// 刚声明的动作必须**贴着"现在"刻线**长出去。
    ///
    /// 这是曾经的 bug：色块按 `execute_at` 画，于是整块右移了一个前摇
    /// （移动 0.25s 的块画在 [0.15, 0.40]），观感就是"一块东西在半空里跳"。
    #[test]
    fn a_fresh_action_starts_at_the_playhead() {
        let now = 7.5;
        let (left, width) =
            block_span(now, timing::MOVE.total(), now, WINDOW_SECONDS).expect("刚声明应当可见");
        assert_eq!(left, 0.0, "声明时刻 = 现在 → 左边界就落在刻线上");
        assert_eq!(width, timing::MOVE.total() / WINDOW_SECONDS * 100.0);
    }

    /// 结算刻线落在块内：移动的前摇占 0.15 / 0.25 = 60%。
    #[test]
    fn resolve_mark_sits_inside_the_block_at_the_windup_share() {
        let percent = resolve_mark_percent(timing::MOVE.windup, timing::MOVE.total());
        assert!(
            (percent - 60.0).abs() < 1e-3,
            "0.15 / 0.25 应当落在块的 60%，实际 {percent}"
        );
        assert_eq!(resolve_mark_percent(0.0, 1.0), 0.0, "零前摇 = 声明即结算");
        assert_eq!(resolve_mark_percent(1.0, 0.0), 0.0, "零时长不除零");
    }

    #[test]
    fn block_span_clips_actions_that_already_started() {
        // 1 秒前声明、总共占 2 秒的行动（比如正在飞的火球）裁到左端，而不是消失
        let (left, width) = block_span(0.0, 2.0, 1.0, 4.0).unwrap();
        assert_eq!(left, 0.0);
        assert_eq!(width, 25.0, "剩下的 1s 占 4s 视野的 25%");
    }

    #[test]
    fn block_span_drops_finished_and_far_actions() {
        assert!(block_span(0.0, 0.5, 1.0, 4.0).is_none(), "已经结束");
        assert!(block_span(9.0, 1.0, 1.0, 4.0).is_none(), "还在视野之外");
        assert!(block_span(1.0, 0.0, 0.0, 4.0).is_none(), "零时长不画");
    }

    /// 整机：两个单位各有一条车道，各自的色块只出现在自己的行里。
    #[test]
    fn each_actor_draws_in_its_own_lane() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<PauseReasons>()
            .init_resource::<HudCache>()
            .add_systems(Update, update_timeline_system);

        let player = app.world_mut().spawn(Faction::Player).id();
        let enemy = app.world_mut().spawn(Faction::Enemy).id();
        let blocks: Vec<Entity> = (0..LANE_POOL)
            .flat_map(|lane| (0..BLOCK_POOL_PER_LANE).map(move |slot| (lane, slot)))
            .map(|(lane, slot)| {
                app.world_mut()
                    .spawn((
                        TimelineBlock { lane, slot },
                        Node::default(),
                        BackgroundColor(Color::NONE),
                    ))
                    .id()
            })
            .collect();
        // 敌人先声明（0.5s），玩家后声明（2.0s）
        for (actor, declared_at) in [(enemy, 0.5), (player, 2.0)] {
            app.world_mut().spawn(ScheduledAction::declared_at(
                actor,
                timing::MOVE,
                declared_at,
            ));
        }

        app.update();

        let block_of = |lane: usize, slot: usize| blocks[lane * BLOCK_POOL_PER_LANE + slot];
        let player_block = app.world().get::<Node>(block_of(0, 0)).unwrap().clone();
        let enemy_block = app.world().get::<Node>(block_of(1, 0)).unwrap().clone();
        assert_eq!(
            player_block.display,
            Display::Flex,
            "玩家行应当有（草案）色块"
        );
        assert_eq!(
            enemy_block.display,
            Display::Flex,
            "敌人行应当有（草案）色块"
        );
        assert!(
            matches!(enemy_block.left, Val::Percent(value) if value < 25.0),
            "0.5s 声明 → 左边界 12.5%，实际 {:?}",
            enemy_block.left
        );
        assert!(
            matches!(player_block.left, Val::Percent(value) if value > 25.0),
            "2.0s 声明 → 左边界 50%，实际 {:?}",
            player_block.left
        );
        let color_of =
            |app: &App, entity: Entity| app.world().get::<BackgroundColor>(entity).unwrap().0;
        assert_ne!(
            color_of(&app, block_of(0, 0)),
            color_of(&app, block_of(1, 0)),
            "两条车道的色块按阵营区分颜色"
        );
        assert_eq!(
            app.world().get::<Node>(block_of(2, 0)).unwrap().display,
            Display::None,
            "没有单位占用的车道不该画色块"
        );
    }

    /// 候场区：就绪但还没声明的人站在那里；一旦声明就离开候场、进入自己的车道。
    #[test]
    fn ready_units_wait_in_the_staging_area() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<PauseReasons>()
            .init_resource::<HudCache>()
            .add_systems(Update, update_timeline_system);

        let player = app
            .world_mut()
            .spawn((Faction::Player, DecisionSlot::Empty))
            .id();
        let chip = app
            .world_mut()
            .spawn((
                TimelineReadyChip { index: 0 },
                Node {
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ))
            .id();
        let block = app
            .world_mut()
            .spawn((
                TimelineBlock { lane: 0, slot: 0 },
                Node::default(),
                BackgroundColor(Color::NONE),
            ))
            .id();

        app.update();
        let display = |app: &App, entity: Entity| app.world().get::<Node>(entity).unwrap().display;
        assert_eq!(
            display(&app, chip),
            Display::Flex,
            "就绪且没声明 → 站在候场区"
        );
        assert_eq!(display(&app, block), Display::None, "没排期就不占横轴");

        app.world_mut()
            .spawn(ScheduledAction::declared_at(player, timing::MOVE, 0.0));
        app.world_mut()
            .entity_mut(player)
            .insert(DecisionSlot::Filled);
        app.update();
        assert_eq!(display(&app, chip), Display::None, "有排期就不再候场");
        assert_eq!(display(&app, block), Display::Flex, "排期画在自己的车道里");
    }

    /// 世界冻结（等玩家输入）时时间轴应当整帧静止；队列一变就必须重画。
    #[test]
    fn frozen_timeline_is_left_alone_until_the_queue_changes() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<PauseReasons>()
            .init_resource::<HudCache>()
            .add_systems(Update, update_timeline_system);
        // 玩家等输入 → 虚拟时间冻结：这是 HUD 最常处的状态
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        let player = app.world_mut().spawn(Faction::Player).id();
        let state = app
            .world_mut()
            .spawn((TimelineStateLabel, Text::new("")))
            .id();
        let block = app
            .world_mut()
            .spawn((
                TimelineBlock { lane: 0, slot: 0 },
                Node::default(),
                BackgroundColor(Color::NONE),
            ))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<Text>(state).unwrap().0,
            "TIMELINE · RUNNING"
        );

        app.world_mut().get_mut::<Text>(state).unwrap().0 = "SENTINEL".to_string();
        app.update();
        assert_eq!(
            app.world().get::<Text>(state).unwrap().0,
            "SENTINEL",
            "冻结且队列没变时不该重写"
        );

        app.world_mut()
            .spawn(ScheduledAction::declared_at(player, timing::MOVE, 0.5));
        app.update();
        assert_eq!(
            app.world().get::<Node>(block).unwrap().display,
            Display::Flex,
            "队列来了新行动就必须画出色块"
        );
    }

    /// 队列空掉之后，隐藏的色块不能留下上一次的 left / width（幽灵色块）。
    #[test]
    fn hidden_blocks_reset_their_geometry() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<PauseReasons>()
            .init_resource::<HudCache>()
            .add_systems(Update, update_timeline_system);

        let player = app.world_mut().spawn(Faction::Player).id();
        let block = app
            .world_mut()
            .spawn((
                TimelineBlock { lane: 0, slot: 0 },
                Node::default(),
                BackgroundColor(Color::NONE),
            ))
            .id();
        let action = app
            .world_mut()
            .spawn(ScheduledAction::declared_at(player, timing::MOVE, 0.5))
            .id();

        app.update();
        let node = app.world().get::<Node>(block).unwrap();
        assert_eq!(node.display, Display::Flex, "队列里有时应当画出色块");
        assert!(
            matches!(node.left, Val::Percent(value) if value > 0.0),
            "色块应当有非零左边界，实际 {:?}",
            node.left
        );

        // 行动结束（实体销毁）：色块回到隐藏，几何也必须归零
        app.world_mut().entity_mut(action).despawn();
        app.update();

        let node = app.world().get::<Node>(block).unwrap();
        assert_eq!(node.display, Display::None);
        assert_eq!(node.left, Val::Percent(0.0), "隐藏时不该留着旧的左边界");
        assert_eq!(node.width, Val::Percent(0.0), "隐藏时不该留着旧的宽度");
    }
}
