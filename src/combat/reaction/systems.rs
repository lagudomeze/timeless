//! 威胁检测：有**敌对**的东西瞄准玩家 → 请求冻结世界，等玩家表态。
//!
//! 窗口就是 [`ReactionSlot`]（挂在被威胁的**玩家**身上，`threat` + `suggestions` +
//! `resolved`），三个时刻：
//!
//! - **开**：玩家身上还没有窗、且有来源瞄着他 → 取最先落脚的那一个当 `threat`；
//! - **断言**：窗在且 `!resolved` → 每帧写 `Pause(THREAT)`；
//! - **关**：`resolved`（玩家表过态）或 `threat` 没了 → 移除窗口。
//!   **关窗不需要玩家"换了那一手"**：忍受伤害也是一种决策。
//!
//! 表态通道是显式的 [`ReactionAnswer`]（技能键 → `Counter`、右键 → `Abandon`）——
//! 旧实现靠"玩家那一手变了没有"推断表态，那个判据在**后摇 / 不可撤行动**期间
//! 永远为假（没槽可声明、也没行动可撤），会把玩家锁死在冻结里。
//!
//! `Threatened` 只是打在威胁源上的**可读标记**（BRP 诊断锚点），不参与判定。

use std::collections::HashSet;

use bevy::prelude::*;

use crate::clock::{PauseRequest, THREAT};
use crate::combat::Faction;
use crate::movement::Cell;
use crate::timeline::Focus;
use crate::timeline::{ActionOf, InputDriven, ScheduledAction};

use super::components::{CounterSuggestion, ReactionSlot, TargetCell, Threatened, Threatens};

/// 每帧检测：**有没有敌对威胁瞄着玩家**，有就开一个反应窗口并按住世界。
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
/// ## 窗口的生死
///
/// - **开**：玩家身上还没有窗口、且有来源瞄着他 → 取**最先落地**的那一个当
///   `threat`（多威胁一次只处理一个），算 `suggestions`，挂 [`ReactionSlot`]；
/// - **断言**：窗口在且 `!resolved` → 每帧写 `Pause(THREAT)`。
///   世界冻结时来源与移动都停在半路，窗口既然开着就该一直开着；
/// - **关**：`resolved`（玩家表过态）或 `threat` 没了（被打断 / 落地 / 销毁）→ 移除窗口。
///   两者都是"没人再要求停表"了，不需要谁去撤销暂停。
///
/// **表态通道**见 [`crate::combat::reaction::resolve_reaction_system`]。
#[allow(clippy::too_many_arguments)]
pub fn detect_threat_system(
    mut commands: Commands,
    threats: Query<(Entity, &ScheduledAction, &Threatens, &ActionOf)>,
    projectiles: Query<(Entity, &TargetCell, &Faction)>,
    mut players: Query<(Entity, &Cell, &Focus, Option<&mut ReactionSlot>), With<InputDriven>>,
    actors: Query<&Faction>,
    catalogue: Res<crate::skills::SkillRegistry>,
    time: Res<Time<Virtual>>,
    mut pause: MessageWriter<PauseRequest>,
) {
    let now = time.elapsed_secs();
    for (player, cell, focus, slot) in &mut players {
        // 被我方阵营"光顾"的格不算威胁（自己人打自己人另有规则）
        let hostile_to = |faction: &Faction| *faction != Faction::Player;
        let threatens_me = |cells: &[Cell]| cells.contains(cell);

        // 本帧瞄着这个玩家的全部来源，连带它的落地时刻
        let mut aiming: Vec<(Entity, f32)> = Vec::new();
        for (entity, schedule, threat, action_of) in &threats {
            if schedule.pending(now)
                && actors.get(action_of.actor()).is_ok_and(hostile_to)
                && threatens_me(&threat.cells)
            {
                aiming.push((entity, schedule.execute_at));
            }
        }
        for (entity, target, faction) in &projectiles {
            if hostile_to(faction) && threatens_me(&[target.0]) {
                // 投射物没有 `execute_at`：它"落地"就是现在，优先级最低
                aiming.push((entity, f32::MAX));
            }
        }

        match slot {
            // 已经有窗口：表态过就关，来源没了也关
            Some(slot) => {
                let threat_gone = !aiming.iter().any(|(entity, _)| *entity == slot.threat);
                if threat_gone {
                    // 威胁自己消失了（打断 / 落地 / 销毁）→ 关窗
                    commands.entity(player).remove::<ReactionSlot>();
                    continue;
                }
                // 表过态的窗口**留着但不再断言**：留着是为了记住"这个来源已经问过了"，
                // 否则下一帧又会当新威胁重新开窗、把世界冻回来。
                if !slot.resolved {
                    pause.write(PauseRequest::Pause(THREAT));
                }
            }
            // 没窗口：有威胁就开一个（取最先落地的那一个）
            None => {
                let Some((threat, _)) = aiming.iter().min_by(|a, b| a.1.total_cmp(&b.1)).copied()
                else {
                    continue;
                };
                let suggestions = counter_suggestions(&catalogue, focus.current);
                debug!("⚔ 敌对威胁逼近玩家：冻结世界等反应");
                commands.entity(player).insert(ReactionSlot {
                    threat,
                    suggestions,
                    resolved: false,
                });
                pause.write(PauseRequest::Pause(THREAT));
            }
        }
    }
}

/// 能拿哪几手反制：**遍历目录里所有 `counter != None` 的技能**。
///
/// 没有任何硬编码的白名单——"翻滚能躲火球"是翻滚自己的 `counter` 字段说的。
/// 付不起的那条**仍然列出来**（`affordable: false`），HUD 画成不可选：
/// 玩家看得见"我本来能用招架，但精力不够"，这比看不见更有信息量。
pub fn counter_suggestions(
    catalogue: &crate::skills::SkillRegistry,
    focus: u32,
) -> Vec<CounterSuggestion> {
    catalogue
        .all()
        .iter()
        .filter_map(|def| {
            let cost = def.counter?;
            let affordable = match cost {
                crate::skills::CounterCost::Free => true,
                crate::skills::CounterCost::Resource(needed) => focus >= needed,
                crate::skills::CounterCost::CancelDecision => true,
            };
            Some(CounterSuggestion {
                ability: def.id,
                cost,
                affordable,
            })
        })
        .collect()
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

/// 玩家表态：**技能键反制 / 右键放弃** → 标记窗口 `resolved`。
///
/// 它只做一件事——把"我表态了"写下来。真正的反制行动由**各技能自己的声明
/// 系统**物化（谁声明谁物化，没有全局派发器），所以本系统不生成任何行动实体。
///
/// 窗口一 `resolved`，[`detect_threat_system`] 下一帧就不再断言
/// `Pause(THREAT)`：**理由消失 = 世界动**（断言式暂停的红利）。
///
/// **右键放弃不减损任何东西**：那一击照常落地——**忍受伤害也是一种决策**。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactionAnswer {
    /// 用这一手反制（技能键）
    Counter(crate::skills::AbilityId),
    /// 放弃这一轮反制（右键）
    Abandon,
}

pub fn resolve_reaction_system(
    mut answers: MessageReader<ReactionAnswer>,
    mut players: Query<&mut ReactionSlot, With<InputDriven>>,
) {
    let answers: Vec<ReactionAnswer> = answers.read().copied().collect();
    if answers.is_empty() {
        return;
    }
    for mut slot in &mut players {
        if slot.resolved {
            continue;
        }
        // 任意一条表态都算数：窗口只问"表态了没有"，不问"选了哪一手"
        if answers.iter().any(|answer| match answer {
            ReactionAnswer::Counter(ability) => {
                slot.suggestions.iter().any(|s| s.ability == *ability)
            }
            ReactionAnswer::Abandon => true,
        }) {
            slot.resolved = true;
            debug!("🛡 玩家已就这次威胁表态：解冻");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::{AbilityCategory, AbilityDef, AbilityId, CombatTags, SkillRegistry};
    use crate::timeline::{ActionTiming, Focus};
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    /// 反应系统的测试不关心载荷的节奏：自己造一个。
    const TEST_TIMING: ActionTiming = ActionTiming::new(0.2, 0.3, 3);

    #[derive(Resource, Default)]
    struct Captured(Vec<PauseRequest>);

    fn capture(mut requests: MessageReader<PauseRequest>, mut captured: ResMut<Captured>) {
        captured.0.extend(requests.read().cloned());
    }

    /// 目录里放两条能当反制的技能（一条白送、一条花 1 点 Focus），外加一条不能的。
    fn catalogue() -> SkillRegistry {
        let mut registry = SkillRegistry::default();
        let def = |id, counter| AbilityDef {
            id,
            category: AbilityCategory::Movement,
            timing: TEST_TIMING,
            targeting: crate::skills::TargetSelector::SelfOnly,
            cost: 0,
            requirements: &[],
            combat: CombatTags::COMMITTED,
            counter,
            power: 0,
        };
        registry.register(def(AbilityId::Roll, Some(crate::skills::CounterCost::Free)));
        registry.register(def(
            AbilityId::Parry,
            Some(crate::skills::CounterCost::Resource(2)),
        ));
        registry.register(def(AbilityId::Move, None)); // 不能当反制
        registry
    }

    fn threat_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .insert_resource(catalogue())
            .init_resource::<Captured>()
            .add_message::<PauseRequest>()
            .add_message::<ReactionAnswer>()
            .add_systems(
                Update,
                (
                    // 表态先落地：这样"按下技能键"那一帧就已经不算"还没表态"，
                    // 世界当帧就能动（与生产流水线里 `resolve` 排在 `detect` 之前一致）
                    resolve_reaction_system,
                    detect_threat_system,
                    mark_threatened_system,
                    capture,
                )
                    .chain(),
            );
        app
    }

    fn spawn_player(app: &mut App, cell: Cell) -> Entity {
        // `Focus` 现在是**挂在单位身上的组件**（每单位一份），不再是全局资源
        app.world_mut()
            .spawn((InputDriven, cell, Faction::Player, Focus::default()))
            .id()
    }

    fn spawn_enemy(app: &mut App) -> Entity {
        app.world_mut().spawn(Faction::Enemy).id()
    }

    fn capture_of(app: &App) -> Vec<PauseRequest> {
        app.world().resource::<Captured>().0.clone()
    }

    fn slot_of(app: &App, player: Entity) -> Option<ReactionSlot> {
        app.world().get::<ReactionSlot>(player).cloned()
    }

    /// 敌对威胁瞄准玩家 → 开窗口 + 请求冻结；来源消失 → 关窗、不再断言。
    #[test]
    fn a_threat_on_the_players_cell_opens_a_window_and_asks_for_a_freeze() {
        let mut app = threat_app();
        let player = spawn_player(&mut app, Cell::new(2, 2));
        let enemy = spawn_enemy(&mut app);
        let action = app
            .world_mut()
            .spawn((
                ActionOf(enemy),
                ScheduledAction::declared_at(TEST_TIMING, 99.0),
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
        let slot = slot_of(&app, player).expect("应当开了反应窗口");
        assert_eq!(slot.threat, action, "窗口记着是哪条威胁");
        assert!(!slot.resolved, "还没表态");

        // 那条行动被撤销 / 打断 / 落地：威胁消失 → 关窗
        app.world_mut().entity_mut(action).despawn();
        app.world_mut().resource_mut::<Captured>().0.clear();
        app.update();
        assert!(slot_of(&app, player).is_none(), "威胁没了就该关窗");
        assert!(
            capture_of(&app).is_empty(),
            "关窗之后不该再断言暂停，原因下一帧自然消失"
        );
    }

    /// **建议列表来自目录**：所有 `counter != None` 的技能都在，付不起的也列出来。
    #[test]
    fn suggestions_come_from_the_catalogue_not_a_hardcoded_list() {
        let registry = catalogue();

        let rich = counter_suggestions(&registry, 5);
        let abilities: Vec<AbilityId> = rich.iter().map(|s| s.ability).collect();
        assert!(abilities.contains(&AbilityId::Roll), "翻滚能当反制");
        assert!(abilities.contains(&AbilityId::Parry), "招架能当反制");
        assert!(
            !abilities.contains(&AbilityId::Move),
            "`counter: None` 的技能不进建议列表"
        );
        assert!(rich.iter().all(|s| s.affordable), "Focus 充足时都付得起");

        // 付不起的那条**仍然列出来**，只是 affordable = false
        let poor = counter_suggestions(&registry, 0);
        let parry = poor
            .iter()
            .find(|s| s.ability == AbilityId::Parry)
            .expect("付不起也要列出来（玩家该看得见这个选项）");
        assert!(!parry.affordable, "Focus 不够 → 标成不可选");
        let roll = poor
            .iter()
            .find(|s| s.ability == AbilityId::Roll)
            .expect("白送的反制永远可选");
        assert!(roll.affordable);
    }

    /// **表态就解冻**：`resolved` 之后不再断言（理由消失 = 世界动）。
    #[test]
    fn answering_the_window_releases_the_freeze() {
        let mut app = threat_app();
        let player = spawn_player(&mut app, Cell::new(0, 0));
        let enemy = spawn_enemy(&mut app);
        app.world_mut().spawn((
            ActionOf(enemy),
            ScheduledAction::declared_at(TEST_TIMING, 99.0),
            Threatens {
                cells: vec![Cell::new(0, 0)],
            },
        ));

        app.update();
        assert!(slot_of(&app, player).is_some_and(|s| !s.resolved));

        // 玩家按技能键反制
        app.world_mut()
            .write_message(ReactionAnswer::Counter(AbilityId::Roll));
        app.world_mut().resource_mut::<Captured>().0.clear();
        // 一帧就够：`resolve` 排在 `detect` 之前，表态当帧生效
        app.update();

        assert!(
            slot_of(&app, player).is_some_and(|s| s.resolved),
            "表态应当写进窗口"
        );
        assert!(
            capture_of(&app).is_empty(),
            "表态之后不该再断言暂停，实际 {:?}",
            capture_of(&app)
        );
    }

    /// 右键放弃：**同样算表态**——那一击照常落地（忍受伤害也是一种决策）。
    #[test]
    fn abandoning_also_counts_as_an_answer() {
        let mut app = threat_app();
        let player = spawn_player(&mut app, Cell::new(0, 0));
        let enemy = spawn_enemy(&mut app);
        app.world_mut().spawn((
            ActionOf(enemy),
            ScheduledAction::declared_at(TEST_TIMING, 99.0),
            Threatens {
                cells: vec![Cell::new(0, 0)],
            },
        ));
        app.update();

        app.world_mut().write_message(ReactionAnswer::Abandon);
        app.update();
        assert!(
            slot_of(&app, player).is_some_and(|s| s.resolved),
            "放弃也是表态（世界该动，那一击该来就来）"
        );
    }

    /// 玩家自己的攻击不是威胁：否则「往自己脚下扔火球」会把世界冻死。
    #[test]
    fn the_players_own_action_is_not_a_threat() {
        let mut app = threat_app();
        let player = spawn_player(&mut app, Cell::new(0, 0));
        app.world_mut().spawn((
            ActionOf(player),
            ScheduledAction::declared_at(TEST_TIMING, 99.0),
            Threatens {
                cells: vec![Cell::new(0, 0)],
            },
        ));

        app.update();
        assert!(
            capture_of(&app).is_empty(),
            "自己瞄自己：不冻世界（否则那发火球永远飞不出去）"
        );
        assert!(slot_of(&app, player).is_none());
    }

    /// 飞行中的敌对投射物也算威胁（它已经出了手）。
    #[test]
    fn a_projectile_aimed_at_the_player_counts_as_a_threat() {
        let mut app = threat_app();
        let player = spawn_player(&mut app, Cell::new(1, 1));
        app.world_mut()
            .spawn((TargetCell(Cell::new(1, 1)), Faction::Enemy));

        app.update();
        assert_eq!(capture_of(&app).last(), Some(&PauseRequest::Pause(THREAT)));
        assert!(slot_of(&app, player).is_some());
    }

    /// 多威胁一次只处理一个：取**最先落地**的那条当 `threat`。
    #[test]
    fn the_window_takes_the_soonest_landing_threat() {
        let mut app = threat_app();
        let player = spawn_player(&mut app, Cell::new(0, 0));
        let enemy = spawn_enemy(&mut app);
        let threat = |app: &mut App, at: f32| {
            app.world_mut()
                .spawn((
                    ActionOf(enemy),
                    ScheduledAction::declared_at(TEST_TIMING, at),
                    Threatens {
                        cells: vec![Cell::new(0, 0)],
                    },
                ))
                .id()
        };
        let _late = threat(&mut app, 50.0);
        let soon = threat(&mut app, 5.0);

        app.update();
        assert_eq!(
            slot_of(&app, player).map(|s| s.threat),
            Some(soon),
            "窗口该取最先落地的那一个"
        );
    }

    /// **多个威胁 = 多次表态**：窗口一次只问一个，但下一个会接着问。
    ///
    /// 这条钉住"多段攻击只会问一次"这个**曾经的担心并不成立**：威胁源是**实体**，
    /// 每个来源各自开窗。三刀同时压过来时，玩家答一刀、那一刀落地，
    /// 下一刀立刻开新窗重新冻住——不会漏问，也不会一次问三遍。
    ///
    /// （真实的多段技能——一条行动打三下——目前还不存在：那需要行动自己声明
    /// 三个落地时刻，属于 `PendingHit` / `ActionTemplate` 的范畴。）
    #[test]
    fn each_of_several_threats_gets_its_own_ask() {
        let mut app = threat_app();
        let player = spawn_player(&mut app, Cell::new(0, 0));
        let enemy = spawn_enemy(&mut app);
        let threat = |app: &mut App, at: f32| {
            app.world_mut()
                .spawn((
                    ActionOf(enemy),
                    ScheduledAction::declared_at(TEST_TIMING, at),
                    Threatens {
                        cells: vec![Cell::new(0, 0)],
                    },
                ))
                .id()
        };
        let first = threat(&mut app, 10.0);
        let second = threat(&mut app, 20.0);

        // 第一个开窗
        app.update();
        assert_eq!(
            slot_of(&app, player).map(|s| s.threat),
            Some(first),
            "先问最先落地的那个"
        );

        // 玩家表态 → 不再断言（世界可以动）
        app.world_mut()
            .write_message(ReactionAnswer::Counter(AbilityId::Roll));
        app.update();
        app.world_mut().resource_mut::<Captured>().0.clear();

        // 第一个落地（被销毁）→ 窗口该交给**下一个**，并且重新冻住
        app.world_mut().entity_mut(first).despawn();
        app.update(); // 这一帧关旧窗（命令延迟落地）
        app.update(); // 下一帧的新窗口

        assert_eq!(
            slot_of(&app, player).map(|s| s.threat),
            Some(second),
            "还有威胁没处理，就该开新窗——不能因为问过一次就放过它"
        );
        assert!(
            capture_of(&app).contains(&PauseRequest::Pause(THREAT)),
            "新的威胁要重新冻住世界，实际 {:?}",
            capture_of(&app)
        );
    }

    /// 没瞄到玩家脚下的格就不算威胁（同一发火球打向别处）。
    #[test]
    fn a_threat_elsewhere_does_not_freeze_the_world() {
        let mut app = threat_app();
        let player = spawn_player(&mut app, Cell::new(0, 0));
        let enemy = spawn_enemy(&mut app);
        app.world_mut().spawn((
            ActionOf(enemy),
            ScheduledAction::declared_at(TEST_TIMING, 99.0),
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
        assert!(slot_of(&app, player).is_none());
    }
}
