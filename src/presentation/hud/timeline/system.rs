//! 时间轴的**系统**：每帧把世界摘成 [`ActionRow`] / 名单 / 就绪表，
//! 交给 [`build_model`] 算快照，再**只在快照变了**的时候把结果写进 UI 节点。
//!
//! 这一层刻意很薄：它只做"取数 → 比对 → 写 UI"，任何"画在哪、多宽"的判断都在
//! [`super::model`]（纯函数、可脱离 App 单测）。

use bevy::prelude::*;

use crate::clock::PauseReasons;
use crate::combat::Faction;
use crate::timeline::{ActionOf, ActionTiming, DecisionSlot, ScheduledAction};

use super::super::{HudCache, faction_color_alpha};
use super::model::{ActionRow, build_model, faction_letter};
use super::scene::{
    TimelineBlock, TimelineBlockLabel, TimelineBlockMark, TimelineLane, TimelineLaneLabel,
    TimelineReadyChip, TimelineReadyLabel, TimelineStateLabel,
};

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
    actors: Query<(Entity, &Faction, Option<&DecisionSlot>)>,
    actions: Query<(&ScheduledAction, &ActionTiming, &ActionOf)>,
    mut cache: ResMut<HudCache>,
    mut states: StateTextQuery<'_, '_>,
    mut lane_labels: LaneLabelQuery<'_, '_>,
    mut blocks: Query<(&TimelineBlock, &mut Node, &mut BackgroundColor)>,
    mut marks: BlockMarkQuery<'_, '_>,
    mut labels: BlockLabelQuery<'_, '_>,
    mut chips: ReadyChipQuery<'_, '_>,
    mut chip_labels: ReadyLabelQuery<'_, '_>,
) {
    let now_seconds = now.elapsed_secs();

    // 取数：世界 → 普通切片（模型层因此可以脱离 App 单测）
    let roster: Vec<(Entity, Faction)> = actors
        .iter()
        .map(|(entity, faction, _)| (entity, *faction))
        .collect();
    let rows: Vec<ActionRow> = actions
        .iter()
        .map(|(schedule, timing, action_of)| ActionRow {
            actor: action_of.actor(),
            // 色块左边界 = **声明时刻** = 执行时刻 − 前摇；声明时刻不再单独存一份，
            // 而是从行动自己的节奏（`ActionTiming`）反推出来。
            declared_at: schedule.execute_at - timing.windup,
            total: timing.total(),
            windup: timing.windup,
            // 还没到点 = 前摇中 = 还能撤（半透明表示"这一手还改得动"）
            draft: schedule.pending(now_seconds),
        })
        .collect();
    // 没有决策槽的单位按空闲算（与旧的 `unwrap_or_default()` 一致）
    let ready: Vec<Entity> = actors
        .iter()
        .filter(|(_, _, slot)| slot.is_none_or(|slot| slot.is_idle()))
        .map(|(entity, _, _)| entity)
        .collect();

    let frozen = reasons.is_frozen().then(|| reasons.labels());
    let model = build_model(frozen.as_deref(), &roster, &rows, &ready, now_seconds);

    // 快照比对：什么都没变就整帧不碰 UI
    if model.matches(&cache.timeline) {
        return;
    }
    model.store(&mut cache.timeline);

    // 写 UI
    for mut text in &mut states {
        **text = model.state.clone();
    }

    for (label, mut text, mut color) in &mut lane_labels {
        let faction = model.lane_faction.get(label.index).copied().flatten();
        **text = faction.map(faction_letter).unwrap_or_default().to_string();
        if let Some(faction) = faction {
            *color = TextColor(faction_color_alpha(faction, 0.9));
        }
    }

    for (block, mut node, mut color) in &mut blocks {
        match model
            .lanes
            .get(block.lane)
            .and_then(|lane| lane.get(block.slot))
        {
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
            model
                .lanes
                .get(mark.lane)
                .and_then(|lane| lane.get(mark.slot))
                .map(|slot| slot.mark)
                .unwrap_or_default(),
        );
    }

    for (label, mut text) in &mut labels {
        **text = model
            .lanes
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
        let faction = model
            .ready
            .contains(&chip.index)
            .then(|| model.lane_faction.get(chip.index).copied().flatten())
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
        **text = model
            .lane_faction
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
    use crate::movement::MOVE_TIMING;
    use crate::presentation::hud::timeline::model::{BLOCK_POOL_PER_LANE, LANE_POOL};
    use crate::timeline::decision::BUSY_SENTINEL;

    fn timeline_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<PauseReasons>()
            .init_resource::<crate::clock::ManualPause>()
            .init_resource::<HudCache>()
            .add_systems(Update, update_timeline_system);
        app
    }

    /// 整机：两个单位各有一条车道，各自的色块只出现在自己的行里。
    #[test]
    fn each_actor_draws_in_its_own_lane() {
        let mut app = timeline_app();

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
            app.world_mut().spawn((
                ActionOf(actor),
                MOVE_TIMING,
                ScheduledAction::declared_at(MOVE_TIMING, declared_at),
            ));
        }
        // 决策槽查的是"有没有、空不空"，两个单位都占着
        for actor in [player, enemy] {
            app.world_mut()
                .entity_mut(actor)
                .insert(DecisionSlot::Executing {
                    until: BUSY_SENTINEL,
                });
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
        let mut app = timeline_app();

        let player = app
            .world_mut()
            .spawn((Faction::Player, DecisionSlot::Idle { intent: None }))
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

        app.world_mut().spawn((
            ActionOf(player),
            MOVE_TIMING,
            ScheduledAction::declared_at(MOVE_TIMING, 0.0),
        ));
        app.world_mut()
            .entity_mut(player)
            .insert(DecisionSlot::Executing {
                until: BUSY_SENTINEL,
            });
        app.update();
        assert_eq!(display(&app, chip), Display::None, "有排期就不再候场");
        assert_eq!(display(&app, block), Display::Flex, "排期画在自己的车道里");
    }

    /// 世界冻结（等玩家输入）时时间轴应当整帧静止；队列一变就必须重画。
    #[test]
    fn frozen_timeline_is_left_alone_until_the_queue_changes() {
        let mut app = timeline_app();
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

        app.world_mut().spawn((
            ActionOf(player),
            MOVE_TIMING,
            ScheduledAction::declared_at(MOVE_TIMING, 0.5),
        ));
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
        let mut app = timeline_app();

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
            .spawn((
                ActionOf(player),
                MOVE_TIMING,
                ScheduledAction::declared_at(MOVE_TIMING, 0.5),
            ))
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
