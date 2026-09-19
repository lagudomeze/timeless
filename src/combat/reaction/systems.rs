//! 威胁检测：有**敌对**的东西瞄准玩家 → 请求冻结世界，等玩家表态。

use std::collections::HashSet;

use bevy::prelude::*;

use crate::combat::Faction;
use crate::movement::Cell;
use crate::timeline::{InputDriven, PauseRequest, ScheduledAction, THREAT};

use super::components::{TargetCell, ThreatWindow, Threatens};

/// 每帧检测：**有没有敌对的东西正打在玩家头上**。
///
/// 判据（两条取或）：
///
/// 1. 有行动**还没到点**（`now < execute_at`）、行动者是敌对阵营、且它的 [`Threatens`]
///    覆盖玩家所在的格——前摇中的火球与近战横扫都在这里被看见；
/// 2. 有飞行中的**敌对**投射物瞄准玩家所在的格（[`TargetCell`]）——已经出了手的那种。
///
/// 「敌对」这一条不能省：玩家自己的火球砸在自己脚下时，那是**自己**说了算的事，
/// 把世界冻住只会让那发火球永远飞不出去。
///
/// 威胁还在、玩家又还没表态时**每帧断言** `Pause("threat")`：世界因此冻着，
/// 玩家可以撤销 / 换手 / 花 1 点 Focus 抢先手。**玩家换了一手就算表态**
/// （见 [`ThreatWindow`]），此后不再断言，原因下一帧自然消失、世界解冻——
/// 否则双方都在冻结里，威胁永远不会自己消失。
#[allow(clippy::too_many_arguments)]
pub fn detect_threat_system(
    threats: Query<(&ScheduledAction, &Threatens, &ChildOf)>,
    actions: Query<(&ScheduledAction, &ChildOf)>,
    projectiles: Query<(&TargetCell, &Faction)>,
    players: Query<(&Cell, &Faction), With<InputDriven>>,
    actors: Query<&Faction>,
    drivers: Query<(), With<InputDriven>>,
    time: Res<Time<Virtual>>,
    mut window: ResMut<ThreatWindow>,
    mut pause: MessageWriter<PauseRequest>,
) {
    let now = time.elapsed_secs();
    let player_cells: HashSet<Cell> = players.iter().map(|(cell, _)| *cell).collect();
    let player_factions: Vec<Faction> = players.iter().map(|(_, faction)| *faction).collect();
    let hostile = |faction: &Faction| !player_factions.contains(faction);

    let threatened = threats.iter().any(|(schedule, threat, child_of)| {
        schedule.pending(now)
            && actors.get(child_of.parent()).is_ok_and(hostile)
            && threat.cells.iter().any(|cell| player_cells.contains(cell))
    }) || projectiles
        .iter()
        .any(|(target, faction)| hostile(faction) && player_cells.contains(&target.0));

    // 玩家眼下这一手（用于判断"表态了没有"）。冻结时虚拟时间不动，
    // 因此只能比实体身份，不能比时间戳。
    let player_action = actions
        .iter()
        .find(|(schedule, child_of)| {
            schedule.pending(now) && drivers.get(child_of.parent()).is_ok()
        })
        .map(|(_, child_of)| child_of.parent());

    // 威胁消失：复位（下一次威胁会重新开窗）。不再断言，原因下一帧自然消失
    if !threatened {
        if window.threatening {
            window.threatening = false;
            window.answered = false;
            window.opening_action = None;
        }
        return;
    }

    // 新威胁：开窗，记下玩家当时那一手
    if !window.threatening {
        window.threatening = true;
        window.answered = false;
        window.opening_action = player_action;
        debug!("⚔ 敌对威胁逼近玩家：冻结世界等反应");
    }

    // 玩家换了一手 = 表态：**停止断言**，让他那一手照常落地
    if !window.answered && window.opening_action != player_action {
        window.answered = true;
        debug!("⚔ 玩家已就这次威胁表态：解冻");
    }

    // 威胁还在、玩家又还没表态 → 这一帧继续把世界按停
    if !window.answered {
        pause.write(PauseRequest::Pause(THREAT));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::ActionTiming;

    /// 反应系统的测试不关心载荷的节奏：自己造一个。
    const TEST_TIMING: ActionTiming = ActionTiming::new(0.2, 0.3, 3);
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    #[derive(Resource, Default)]
    struct Captured(Vec<PauseRequest>);

    fn capture(mut requests: MessageReader<PauseRequest>, mut captured: ResMut<Captured>) {
        captured.0.extend(requests.read().cloned());
    }

    fn threat_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .init_resource::<ThreatWindow>()
            .init_resource::<Captured>()
            .add_message::<PauseRequest>()
            .add_systems(Update, (detect_threat_system, capture).chain());
        app
    }

    fn spawn_player(app: &mut App, cell: Cell) -> Entity {
        app.world_mut()
            .spawn((InputDriven, cell, Faction::Player))
            .id()
    }

    fn spawn_enemy(app: &mut App) -> Entity {
        app.world_mut().spawn(Faction::Enemy).id()
    }

    fn capture_of(app: &App) -> Vec<PauseRequest> {
        app.world().resource::<Captured>().0.clone()
    }

    /// 敌对威胁瞄准玩家 → 请求冻结；威胁消失 → 请求解冻。
    #[test]
    fn a_threat_on_the_players_cell_asks_for_a_freeze() {
        let mut app = threat_app();
        let player = spawn_player(&mut app, Cell::new(2, 2));
        let enemy = spawn_enemy(&mut app);
        let action = app
            .world_mut()
            .spawn((
                ChildOf(enemy),
                ScheduledAction::declared_at(TEST_TIMING, 0.0),
                Threatens {
                    cells: vec![Cell::new(2, 2)],
                },
            ))
            .id();

        app.update();
        assert_eq!(
            capture_of(&app).last(),
            Some(&PauseRequest::Pause(THREAT)),
            "有人瞄着玩家脚下的格 → 冻住世界"
        );

        // 那条行动被撤销 / 打断 / 落地：威胁消失 → 不再断言，世界因此解冻
        app.world_mut().entity_mut(action).despawn();
        app.world_mut().resource_mut::<Captured>().0.clear();
        app.update();
        assert!(
            capture_of(&app).is_empty(),
            "威胁没了就不该再断言暂停，原因下一帧自然消失"
        );
        assert!(app.world().get_entity(player).is_ok());
    }

    /// 玩家自己的攻击不是威胁：否则「往自己脚下扔火球」会把世界冻死。
    #[test]
    fn the_players_own_action_is_not_a_threat() {
        let mut app = threat_app();
        let player = spawn_player(&mut app, Cell::new(0, 0));
        app.world_mut().spawn((
            ChildOf(player),
            ScheduledAction::declared_at(TEST_TIMING, 0.0),
            Threatens {
                cells: vec![Cell::new(0, 0)],
            },
        ));

        app.update();

        assert!(
            capture_of(&app).is_empty(),
            "自己瞄自己：不冻世界（否则那发火球永远飞不出去）"
        );
    }

    /// 飞行中的敌对投射物也算威胁（它已经出了手）。
    #[test]
    fn a_projectile_aimed_at_the_player_counts_as_a_threat() {
        let mut app = threat_app();
        spawn_player(&mut app, Cell::new(1, 1));
        app.world_mut()
            .spawn((TargetCell(Cell::new(1, 1)), Faction::Enemy));

        app.update();

        assert_eq!(capture_of(&app).last(), Some(&PauseRequest::Pause(THREAT)));
    }

    /// **玩家表态就解冻**：否则双方都冻着，威胁永远不消失（死锁）。
    #[test]
    fn answering_the_threat_releases_the_freeze() {
        let mut app = threat_app();
        let player = spawn_player(&mut app, Cell::new(0, 0));
        let enemy = spawn_enemy(&mut app);
        app.world_mut().spawn((
            ChildOf(enemy),
            ScheduledAction::declared_at(TEST_TIMING, 0.0),
            Threatens {
                cells: vec![Cell::new(0, 0)],
            },
        ));

        app.update(); // 窗口打开：玩家此刻没有行动
        let window = *app.world().resource::<ThreatWindow>();
        assert!(window.threatening && !window.answered);
        assert_eq!(
            capture_of(&app).last(),
            Some(&PauseRequest::Pause(THREAT)),
            "还没表态就一直断言着暂停"
        );

        // 玩家举起一招（换了一手）
        app.world_mut().spawn((
            ChildOf(player),
            ScheduledAction::declared_at(TEST_TIMING, 1.0),
        ));
        app.world_mut().resource_mut::<Captured>().0.clear();
        app.update();

        assert!(
            capture_of(&app).is_empty(),
            "玩家已经就这次威胁表态过了，不该再断言暂停"
        );
        let window = *app.world().resource::<ThreatWindow>();
        assert!(
            window.threatening && window.answered,
            "表态过就不再重复开窗（否则世界会走一帧停一帧）"
        );
    }

    /// 没瞄到玩家脚下的格就不算威胁（同一发火球打向别处）。
    #[test]
    fn a_threat_elsewhere_does_not_freeze_the_world() {
        let mut app = threat_app();
        spawn_player(&mut app, Cell::new(0, 0));
        let enemy = spawn_enemy(&mut app);
        app.world_mut().spawn((
            ChildOf(enemy),
            ScheduledAction::declared_at(TEST_TIMING, 0.0),
            Threatens {
                cells: vec![Cell::new(5, 5)],
            },
        ));

        app.update();

        assert!(capture_of(&app).is_empty(), "打别处不该惊动玩家");
    }
}
