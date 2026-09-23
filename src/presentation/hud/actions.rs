//! 单位面板的最后一行：**当前行动**（`act: fireball`，空闲时 `-`）。
//!
//! 调度器按设计不感知载荷（见 [`crate::timeline`]），所以这里由表现层代它读一次
//! 载荷标记，只把「这条行动是什么」翻成人话——HUD 依然只读游戏状态。
//!
//! **前摇中会带上剩余秒数**（`act: fireball (windup 0.2s)`）：那是**还撤得掉**、
//! 也是**还能躲开**的那段时间窗口（`now < execute_at`）。这是信息层的第一块读数
//! （`docs/game-design.md`「信息即力量」的"帧窗口细节"）——玩家据此决定
//! 要不要右键改主意、要不要抢在它前面动。

use bevy::prelude::*;

use crate::combat::Faction;
use crate::combat::attack::{FireballAction, MeleeAction, ShootAction};
use crate::combat::defense::ParryAction;
use crate::movement::{JumpAction, MoveAction, RollAction};
use crate::timeline::{ActionOf, ScheduledAction};

use super::HudCache;

/// 面板上的行动行标记。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct ActionLabel {
    pub faction: Faction,
}

/// 行动行快照缓存：与上一帧完全相同就整帧不碰 UI。
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ActionLabelCache {
    labels: [String; 2],
}

/// 阵营 → 快照下标。
fn slot(faction: Faction) -> usize {
    match faction {
        Faction::Player => 0,
        Faction::Enemy => 1,
    }
}

/// 把「这个单位现在挂着的行动」写进文本。
#[allow(clippy::too_many_arguments)]
pub fn update_action_labels_system(
    now: Res<Time<Virtual>>,
    units: Query<(Entity, &Faction)>,
    actions: Query<(Entity, &ScheduledAction, &ActionOf)>,
    movements: Query<&MoveAction>,
    jumps: Query<&JumpAction>,
    rolls: Query<&RollAction>,
    parries: Query<&ParryAction>,
    shoots: Query<&ShootAction>,
    fireballs: Query<&FireballAction>,
    melees: Query<&MeleeAction>,
    mut cache: ResMut<HudCache>,
    mut labels: Query<(&ActionLabel, &mut Text)>,
) {
    let now_seconds = now.elapsed_secs();
    // 先算快照（纯读），再决定要不要写
    let mut snapshot = cache.actions.labels.clone();
    for (label, _) in &labels {
        snapshot[slot(label.faction)] = action_text(
            label.faction,
            now_seconds,
            &units,
            &actions,
            &movements,
            &jumps,
            &rolls,
            &parries,
            &shoots,
            &fireballs,
            &melees,
        );
    }
    if cache.actions.labels == snapshot {
        return;
    }
    cache.actions.labels.clone_from(&snapshot);

    for (label, mut text) in &mut labels {
        **text = snapshot[slot(label.faction)].clone();
    }
}

/// 某个阵营这一帧该显示的行动文案。
#[allow(clippy::too_many_arguments)]
fn action_text(
    faction: Faction,
    now: f32,
    units: &Query<(Entity, &Faction)>,
    actions: &Query<(Entity, &ScheduledAction, &ActionOf)>,
    movements: &Query<&MoveAction>,
    jumps: &Query<&JumpAction>,
    rolls: &Query<&RollAction>,
    parries: &Query<&ParryAction>,
    shoots: &Query<&ShootAction>,
    fireballs: &Query<&FireballAction>,
    melees: &Query<&MeleeAction>,
) -> String {
    let Some(actor) = units
        .iter()
        .find(|(_, unit_faction)| **unit_faction == faction)
        .map(|(entity, _)| entity)
    else {
        return "down".to_string();
    };
    // 行动者 = 行动实体记着的归属方
    let Some((action, schedule, _)) = actions
        .iter()
        .find(|(_, _, action_of)| action_of.actor() == actor)
    else {
        return "act: -".to_string();
    };
    let name = payload_name(
        action, movements, jumps, rolls, parries, shoots, fireballs, melees,
    );
    if schedule.pending(now) {
        // 前摇剩余秒数：信息层的"帧窗口细节"——还剩多久这一手就落地
        // （满前摇 = `execute_at - declared_at`，当前进度由 `now` 决定）
        let remaining = (schedule.execute_at - now).max(0.0);
        format!("act: {name} (windup {remaining:.1}s)")
    } else {
        format!("act: {name}")
    }
}

/// 行动实体 → 载荷名（找不到就退回 `action`）。
#[allow(clippy::too_many_arguments)]
fn payload_name(
    action: Entity,
    movements: &Query<&MoveAction>,
    jumps: &Query<&JumpAction>,
    rolls: &Query<&RollAction>,
    parries: &Query<&ParryAction>,
    shoots: &Query<&ShootAction>,
    fireballs: &Query<&FireballAction>,
    melees: &Query<&MeleeAction>,
) -> &'static str {
    if movements.get(action).is_ok() {
        "move"
    } else if jumps.get(action).is_ok() {
        "jump"
    } else if rolls.get(action).is_ok() {
        "roll"
    } else if parries.get(action).is_ok() {
        "parry"
    } else if fireballs.get(action).is_ok() {
        "fireball"
    } else if shoots.get(action).is_ok() {
        "shoot"
    } else if melees.get(action).is_ok() {
        "melee"
    } else {
        "action"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movement::{JUMP_TIMING, MOVE_TIMING};

    fn label_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<HudCache>()
            .add_systems(Update, update_action_labels_system);
        app
    }

    fn spawn_unit(app: &mut App, faction: Faction) -> Entity {
        app.world_mut().spawn(faction).id()
    }

    fn spawn_label(app: &mut App, faction: Faction) -> Entity {
        app.world_mut()
            .spawn((ActionLabel { faction }, Text::new("")))
            .id()
    }

    fn text_of(app: &App, entity: Entity) -> String {
        app.world().get::<Text>(entity).unwrap().0.clone()
    }

    /// 空闲单位显示 `-`。
    #[test]
    fn idle_unit_shows_a_dash() {
        let mut app = label_app();
        spawn_unit(&mut app, Faction::Player);
        let label = spawn_label(&mut app, Faction::Player);

        app.update();

        assert_eq!(text_of(&app, label), "act: -");
    }

    /// 挂着行动时显示载荷名；**前摇中**带上剩余秒数——那正是还能撤、还能躲的窗口。
    #[test]
    fn pending_action_is_named_and_windups_show_the_time_left() {
        let mut app = label_app();
        let player = spawn_unit(&mut app, Faction::Player);
        let label = spawn_label(&mut app, Faction::Player);
        let action = app
            .world_mut()
            .spawn((
                ActionOf(player),
                MoveAction::default(),
                ScheduledAction::declared_at(MOVE_TIMING, 0.0),
            ))
            .id();

        app.update();
        // `MOVE_TIMING.windup = 0.15`，声明于 0.0；首帧剩余即接近满前摇
        let shown = text_of(&app, label);
        assert!(shown.starts_with("act: move (windup"), "{shown}");
        assert!(shown.ends_with("s)"), "前摇读数要带剩余秒数与单位：{shown}");

        // 世界走过前摇：这条行动已经落地（等执行器收拾），不再标注 windup
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .advance_by(std::time::Duration::from_secs(1));
        app.update();
        assert_eq!(text_of(&app, label), "act: move");
        assert!(app.world().get_entity(action).is_ok());
    }

    /// **倒计时真的在走**：同一手行动，虚拟时间前进之后剩余秒数必须变小。
    ///
    /// 这条守的是"读数有没有真的连到 `execute_at`"——只显示一个静态的 `(windup)`
    /// 也能通过上一条测试，但那样玩家读不到"还剩多久"。
    #[test]
    fn the_windup_readout_counts_down_as_time_passes() {
        let mut app = label_app();
        let player = spawn_unit(&mut app, Faction::Player);
        let label = spawn_label(&mut app, Faction::Player);
        app.world_mut().spawn((
            ActionOf(player),
            MoveAction::default(),
            ScheduledAction::declared_at(MOVE_TIMING, 0.0),
        ));

        let seconds = |app: &App| -> f32 {
            let text = app.world().get::<Text>(label).unwrap().0.clone();
            text.rsplit_once("windup ")
                .and_then(|(_, rest)| rest.trim_end_matches(['s', ')']).parse().ok())
                .unwrap_or_else(|| panic!("读不出前摇剩余秒数：{text}"))
        };

        app.update();
        let before = seconds(&app);
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .advance_by(std::time::Duration::from_millis(60));
        app.update();
        let after = seconds(&app);

        assert!(
            after < before,
            "时间前进了，前摇剩余应当变小：{before} → {after}"
        );
    }

    /// 单位阵亡（实体没了）时显示 `down`，不留下过期的行动名。
    #[test]
    fn missing_unit_shows_down() {
        let mut app = label_app();
        let label = spawn_label(&mut app, Faction::Enemy);

        app.update();

        assert_eq!(text_of(&app, label), "down");
    }

    /// 行动没变时整帧不碰 UI（拿哨兵值当探针）。
    #[test]
    fn action_labels_are_left_alone_when_nothing_changes() {
        let mut app = label_app();
        let player = spawn_unit(&mut app, Faction::Player);
        let label = spawn_label(&mut app, Faction::Player);

        app.update();
        assert_eq!(text_of(&app, label), "act: -");

        app.world_mut().get_mut::<Text>(label).unwrap().0 = "SENTINEL".to_string();
        app.update();
        assert_eq!(
            text_of(&app, label),
            "SENTINEL",
            "数据没变就不该被系统盖回去"
        );

        app.world_mut().spawn((
            ActionOf(player),
            JumpAction,
            ScheduledAction::declared_at(JUMP_TIMING, 0.0),
        ));
        app.update();
        let shown = text_of(&app, label);
        assert!(
            shown.starts_with("act: jump (windup"),
            "换了行动就必须重写：{shown}"
        );
    }
}
