//! 威胁检测：有**敌对**的东西瞄准玩家 → 请求冻结世界，等玩家表态。
//!
//! 窗口的生命周期由 [`ThreatWindow`] 这个状态机维护（开 / 维持 / 关），
//! 关键是**关窗不需要玩家"换了那一手"**：
//!
//! - 玩家按空格（`PauseRequest::Toggle`）清空原因集合 → 世界放开，
//!   那一击照常落地（**忍受伤害也是一种决策**）；
//! - 旧实现靠"玩家那一手变了没有"推断表态，那个判据在**后摇 / 不可撤行动**期间
//!   永远为假（没槽可声明、也没行动可撤），会把玩家锁死在冻结里。
//!
//! `Threatened` 只是打在威胁源上的**可读标记**（BRP 诊断锚点），不参与判定。

use std::collections::HashSet;

use bevy::prelude::*;

use crate::clock::{PauseReasons, PauseRequest, THREAT};
use crate::combat::Faction;
use crate::movement::Cell;
use crate::timeline::{ActionOf, InputDriven, ScheduledAction};

use super::components::{TargetCell, ThreatWindow, Threatened, Threatens};

/// 每帧检测：**有没有敌对威胁瞄着玩家**，有就把世界按住、等玩家表态。
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
/// ## 窗口怎么开、怎么关
///
/// 窗口状态由本域自己维护（[`ThreatWindow`]），只把"玩家有没有放开世界"
/// 当作外部信号：`PauseRequest::Toggle` 会在 [`apply_pause_toggles_system`] 里
/// **清空**原因集合（那一步排在各领域断言之前），于是本系统读到"集合里没有 `THREAT`"
/// 就知道玩家放行了。
///
/// 三条规则：
///
/// - **开**：命中判据、且窗口没开 → 断言 `Pause(THREAT)`；
/// - **维持**：窗口开着期间，只要还有来源瞄着玩家就继续断言——世界冻结时移动与
///   火球都停在半路，窗口既然开着就该一直开着（否则玩家来不及反应）；
/// - **关**：集合里没有 `THREAT` 了（玩家放行）→ 关窗并记 `dismissed`，
///   **同一次威胁不再重开**。那一击照常落地——**忍受伤害也是一种决策**。
///
/// [`apply_pause_toggles_system`]: crate::timeline::apply_pause_toggles_system
#[allow(clippy::too_many_arguments)]
pub fn detect_threat_system(
    threats: Query<(Entity, &ScheduledAction, &Threatens, &ActionOf)>,
    projectiles: Query<(Entity, &TargetCell, &Faction)>,
    players: Query<(&Cell, &Faction), With<InputDriven>>,
    actors: Query<&Faction>,
    time: Res<Time<Virtual>>,
    open: Res<PauseReasons>,
    mut window: ResMut<ThreatWindow>,
    mut pause: MessageWriter<PauseRequest>,
) {
    let now = time.elapsed_secs();
    let player_cells: HashSet<Cell> = players.iter().map(|(cell, _)| *cell).collect();
    let player_factions: Vec<Faction> = players.iter().map(|(_, faction)| *faction).collect();
    let hostile = |faction: &Faction| !player_factions.contains(faction);
    let threatens_player = |cells: &[Cell]| cells.iter().any(|cell| player_cells.contains(cell));

    // 本帧仍瞄着玩家的来源（不管有没有惊动过）
    let mut aiming: Vec<Entity> = Vec::new();
    for (entity, schedule, threat, action_of) in &threats {
        if schedule.pending(now)
            && actors.get(action_of.actor()).is_ok_and(hostile)
            && threatens_player(&threat.cells)
        {
            aiming.push(entity);
        }
    }
    for (entity, target, faction) in &projectiles {
        if hostile(faction) && threatens_player(&[target.0]) {
            aiming.push(entity);
        }
    }

    // 玩家放开了世界（原因集合里没有 THREAT 了）→ 关窗，并记住"这次别重开"
    if window.open && !open.contains(THREAT) {
        window.open = false;
        window.dismissed = true;
    }

    if !window.open && !window.dismissed && !aiming.is_empty() {
        window.open = true;
        debug!("⚔ 敌对威胁逼近玩家：冻结世界等反应");
    }

    if window.open {
        // 窗口开着期间维持冻结；来源全没了就自然关掉
        if aiming.is_empty() {
            window.open = false;
            window.dismissed = false;
        } else {
            pause.write(PauseRequest::Pause(THREAT));
        }
    }
}

/// 把本帧仍瞄着玩家的来源落成 [`Threatened`] 标记（可读的诊断锚点）——排在检测之后。
///
/// 同帧写入会让检测系统自己漏检（它查的是"还没有标记的"），所以拆成两个系统：
/// 检测只读、打标记只写。来源被销毁时标记随实体一起走，不需要清理。
pub fn mark_threatened_system(
    mut commands: Commands,
    threats: Query<(Entity, &ScheduledAction, &Threatens, &ActionOf), Without<Threatened>>,
    projectiles: Query<(Entity, &TargetCell, &Faction), Without<Threatened>>,
    players: Query<(&Cell, &Faction), With<InputDriven>>,
    actors: Query<&Faction>,
    time: Res<Time<Virtual>>,
) {
    let now = time.elapsed_secs();
    let player_cells: HashSet<Cell> = players.iter().map(|(cell, _)| *cell).collect();
    let player_factions: Vec<Faction> = players.iter().map(|(_, faction)| *faction).collect();
    let hostile = |faction: &Faction| !player_factions.contains(faction);
    let threatens_player = |cells: &[Cell]| cells.iter().any(|cell| player_cells.contains(cell));

    for (entity, schedule, threat, action_of) in &threats {
        if schedule.pending(now)
            && actors.get(action_of.actor()).is_ok_and(hostile)
            && threatens_player(&threat.cells)
        {
            commands.entity(entity).insert(Threatened);
        }
    }
    for (entity, target, faction) in &projectiles {
        if hostile(faction) && threatens_player(&[target.0]) {
            commands.entity(entity).insert(Threatened);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::ActionTiming;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    /// 反应系统的测试不关心载荷的节奏：自己造一个。
    const TEST_TIMING: ActionTiming = ActionTiming::new(0.2, 0.3, 3);

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
            .init_resource::<PauseReasons>()
            .init_resource::<crate::clock::ManualPause>()
            .init_resource::<Captured>()
            .add_message::<PauseRequest>()
            .add_systems(
                Update,
                (detect_threat_system, mark_threatened_system, capture).chain(),
            );
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
                ActionOf(enemy),
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
            ActionOf(player),
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

    /// **窗口开着时持续断言**：世界冻结时威胁源与玩家的位移都停在半路，
    /// 所以只要窗口还开着、来源还在，就该一直按住。
    ///
    /// 这也正是"回合制等玩家决策"能成立的原因——只冻一帧的话，玩家根本来不及反应。
    #[test]
    fn an_open_window_keeps_asserting_while_the_threat_remains() {
        let mut app = threat_app();
        spawn_player(&mut app, Cell::new(0, 0));
        let enemy = spawn_enemy(&mut app);
        // 执行时刻排到很远：这条威胁在整个测试期间都"还没到点"
        // （用 `TEST_TIMING` 声明的话，0.2s 的前摇会被 0.1s/帧的手动时钟走到点）
        let action = app
            .world_mut()
            .spawn((
                ActionOf(enemy),
                ScheduledAction::declared_at(TEST_TIMING, 99.0),
                Threatens {
                    cells: vec![Cell::new(0, 0)],
                },
            ))
            .id();

        for frame in 0..5 {
            app.world_mut().resource_mut::<Captured>().0.clear();
            // 冒充时间线：窗口开着 → 原因集合里就有 THREAT（真实现里是
            // `process_pause_requests` 收下断言）
            app.world_mut()
                .resource_mut::<PauseReasons>()
                .insert(THREAT);
            app.update();
            assert_eq!(
                capture_of(&app).last(),
                Some(&PauseRequest::Pause(THREAT)),
                "第 {frame} 帧：窗口还开着就该继续按住世界"
            );
        }
        assert!(app.world().get::<Threatened>(action).is_some());

        // 来源消失（打断 / 落地）→ 窗口关掉，不再断言
        app.world_mut().entity_mut(action).despawn();
        app.world_mut().resource_mut::<Captured>().0.clear();
        app.update();
        assert!(capture_of(&app).is_empty(), "来源没了就不该再断言暂停");
        assert!(!app.world().resource::<ThreatWindow>().open, "窗口该关上了");
    }

    /// **玩家放开世界（原因集合里不再有 THREAT）→ 窗口关闭，同一次威胁不再重开。**
    ///
    /// 这条是「玩家总能走出去」的根据：世界一放开，同一个来源不会立刻冻回来，
    /// 那一击照常落地——代价自负（忍受伤害也是一种决策）。
    #[test]
    fn a_dismissed_window_does_not_reopen_for_the_same_threat() {
        let mut app = threat_app();
        spawn_player(&mut app, Cell::new(0, 0));
        let enemy = spawn_enemy(&mut app);
        app.world_mut().spawn((
            ActionOf(enemy),
            ScheduledAction::declared_at(TEST_TIMING, 0.0),
            Threatens {
                cells: vec![Cell::new(0, 0)],
            },
        ));

        // 窗口开着
        app.world_mut()
            .resource_mut::<PauseReasons>()
            .insert(THREAT);
        app.update();
        assert!(app.world().resource::<ThreatWindow>().open);

        // 玩家按空格：时间线清空原因集合，下一帧检测系统读到"没有 THREAT"
        app.world_mut().resource_mut::<PauseReasons>().clear();
        app.world_mut().resource_mut::<Captured>().0.clear();
        app.update();
        let window = *app.world().resource::<ThreatWindow>();
        assert!(!window.open && window.dismissed, "放开之后窗口该关上并记住");

        // 威胁**还在**瞄着玩家，但不该再冻回来
        for frame in 0..5 {
            app.world_mut().resource_mut::<Captured>().0.clear();
            app.update();
            assert!(
                capture_of(&app).is_empty(),
                "第 {frame} 帧：同一次威胁被玩家放开过，不该重开"
            );
        }
    }

    /// 没瞄到玩家脚下的格就不算威胁（同一发火球打向别处）。
    #[test]
    fn a_threat_elsewhere_does_not_freeze_the_world() {
        let mut app = threat_app();
        spawn_player(&mut app, Cell::new(0, 0));
        let enemy = spawn_enemy(&mut app);
        app.world_mut().spawn((
            ActionOf(enemy),
            ScheduledAction::declared_at(TEST_TIMING, 0.0),
            Threatens {
                cells: vec![Cell::new(5, 5)],
            },
        ));

        app.update();

        assert!(
            capture_of(&app).is_empty(),
            "打别处不该惊动玩家：{:?}",
            capture_of(&app)
        );
    }

    /// 【临时探针】窗口维持：为什么不持续断言？
    #[test]
    fn probe_why_not_persistent() {
        use crate::clock::PauseReasons;
        let mut app = threat_app();
        app.init_resource::<PauseReasons>()
            .init_resource::<crate::clock::ManualPause>();
        // 冒充时间线：把 Pause 断言收进集合（真实现里是 process_pause_requests）
        fn collect(mut r: MessageReader<PauseRequest>, mut reasons: ResMut<PauseReasons>) {
            reasons.clear();
            for req in r.read() {
                if let PauseRequest::Pause(x) = req {
                    reasons.insert(x);
                }
            }
        }
        app.add_systems(Update, collect.after(mark_threatened_system));

        let player = spawn_player(&mut app, Cell::new(0, 0));
        let enemy = spawn_enemy(&mut app);
        let action = app
            .world_mut()
            .spawn((
                ActionOf(enemy),
                ScheduledAction::declared_at(TEST_TIMING, 0.0),
                Threatens {
                    cells: vec![Cell::new(0, 0)],
                },
            ))
            .id();
        let _ = player;
        for frame in 0..4 {
            app.update();
            println!(
                "探针 第{frame}帧：原因={:?} 有标记={}",
                app.world().resource::<PauseReasons>().labels(),
                app.world().get::<Threatened>(action).is_some()
            );
        }
    }
}
