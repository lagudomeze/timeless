//! 行动实体 → **载荷名**（`fireball` / `move` / `wait` …）。
//!
//! 调度器按设计不感知载荷（见 [`crate::timeline`]），所以这里由表现层代它读一次
//! 载荷标记，只把「这条行动是什么」翻成人话——HUD 依然只读游戏状态。
//!
//! **一处判据**：单位面板的 `act:` 行与时间轴的悬停读数都走
//! [`payload_name`]，所以同一个动作在两处**不可能叫不同名字**。
//! 面板那一行的排版（`act: fireball (windup 0.3s)`）住在
//! [`UnitPanels::action_text`](super::panels::model::UnitPanels::action_text)——
//! 它按**面板槽**（行）取数，不再按阵营（2026-09-27 修：按阵营会让多敌人面板的
//! 三行显示同一句话，见 `docs/backlog/hud.md`）。

use bevy::prelude::*;

use crate::combat::attack::{FireballAction, MeleeAction, ShootAction};
use crate::combat::defense::ParryAction;
use crate::movement::{JumpAction, MoveAction, RollAction};
use crate::timeline::WaitAction;

/// 行动实体 → 载荷名（找不到就退回 `action`）。
///
/// **公开**：时间轴的悬停读数要用同一套判据（`presentation::hud::timeline`），
/// 否则同一个动作在面板上叫 `fireball`、在时间轴上叫别的名字。
///
/// ⚠️ **每个载荷都要有分支**：漏一个就落到兜底的 `"action"`，面板上会显示
/// `act: action`——玩家按了空格却读不出"我在等"（2026-09-27 实机抓到，
/// 见 [`docs/playtest-checklist.md`](../../../docs/playtest-checklist.md) 第 1 节）。
/// 兜底只该是"以后新增的载荷还没取名"的临时状态，不该是常态。
#[allow(clippy::too_many_arguments)]
pub fn payload_name(
    action: Entity,
    movements: &Query<&MoveAction>,
    jumps: &Query<&JumpAction>,
    rolls: &Query<&RollAction>,
    parries: &Query<&ParryAction>,
    shoots: &Query<&ShootAction>,
    fireballs: &Query<&FireballAction>,
    melees: &Query<&MeleeAction>,
    waits: &Query<&WaitAction>,
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
    } else if waits.get(action).is_ok() {
        // 等待也是一个"动作"：按空格占住决策槽、世界继续跑。
        // 不给它名字就会显示成 `act: action`，等于没说。
        "wait"
    } else {
        "action"
    }
}

/// 载荷查询组：`payload_name` 要的那八个查询绑在一起。
///
/// 它们全是只读的标记查询，`SystemParam` 派生把它们收成一个参数——
/// 否则 `payload_name` 的调用方（面板行 / 时间轴悬停读数）都要在签名里
/// 照抄八行同样的 `Query`。
#[derive(bevy::ecs::system::SystemParam)]
pub struct PayloadQueries<'w, 's> {
    pub movements: Query<'w, 's, &'static MoveAction>,
    pub jumps: Query<'w, 's, &'static JumpAction>,
    pub rolls: Query<'w, 's, &'static RollAction>,
    pub parries: Query<'w, 's, &'static ParryAction>,
    pub shoots: Query<'w, 's, &'static ShootAction>,
    pub fireballs: Query<'w, 's, &'static FireballAction>,
    pub melees: Query<'w, 's, &'static MeleeAction>,
    pub waits: Query<'w, 's, &'static WaitAction>,
}

impl PayloadQueries<'_, '_> {
    /// 见 [`payload_name`]。
    pub fn name_of(&self, action: Entity) -> &'static str {
        payload_name(
            action,
            &self.movements,
            &self.jumps,
            &self.rolls,
            &self.parries,
            &self.shoots,
            &self.fireballs,
            &self.melees,
            &self.waits,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一条挂着某个载荷的行动实体。
    type Spawner = fn(&mut World, Entity) -> Entity;

    /// 待取名的行动实体，以及取到的名字（用资源传进传出，避免测试里手抄查询）。
    #[derive(Resource, Default)]
    struct ToName(Vec<Entity>);
    #[derive(Resource, Default)]
    struct Names(Vec<&'static str>);

    fn name_of(payloads: PayloadQueries, to: Res<ToName>, mut out: ResMut<Names>) {
        out.0 =
            to.0.iter()
                .map(|entity| payloads.name_of(*entity))
                .collect();
    }

    /// 造一个只装"命名器"的 App，返回它和行动者的实体。
    fn naming_app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ToName>()
            .init_resource::<Names>()
            .add_systems(Update, name_of);
        let actor = app.world_mut().spawn_empty().id();
        (app, actor)
    }

    /// **每个载荷都要有自己的名字**——漏一个就落到兜底的 `"action"`。
    ///
    /// 这条守着一次真实退化：等待动作没登记，玩家按空格只看到 `act: action`
    /// （2026-09-27 实机抓到）。面板与时间轴都用这一处命名，所以这里错了两处一起错。
    #[test]
    fn every_payload_has_a_name_of_its_own() {
        let cases: [(&str, Spawner); 8] = [
            ("move", |w, a| {
                w.spawn((crate::timeline::ActionOf(a), MoveAction::default()))
                    .id()
            }),
            ("jump", |w, a| {
                w.spawn((crate::timeline::ActionOf(a), JumpAction)).id()
            }),
            ("roll", |w, a| {
                w.spawn((crate::timeline::ActionOf(a), RollAction::default()))
                    .id()
            }),
            ("parry", |w, a| {
                w.spawn((crate::timeline::ActionOf(a), ParryAction::default()))
                    .id()
            }),
            ("shoot", |w, a| {
                w.spawn((crate::timeline::ActionOf(a), ShootAction)).id()
            }),
            ("fireball", |w, a| {
                w.spawn((crate::timeline::ActionOf(a), FireballAction::default()))
                    .id()
            }),
            ("melee", |w, a| {
                w.spawn((crate::timeline::ActionOf(a), MeleeAction)).id()
            }),
            ("wait", |w, a| {
                w.spawn((crate::timeline::ActionOf(a), WaitAction)).id()
            }),
        ];

        // 一次全造出来，再让命名器一次问完（`SystemParam` 每个 App 只能取一次）
        let (mut app, actor) = naming_app();
        let expected: Vec<&str> = cases.iter().map(|(name, _)| *name).collect();
        let entities: Vec<Entity> = cases
            .iter()
            .map(|(_, spawn)| spawn(app.world_mut(), actor))
            .collect();
        app.world_mut().resource_mut::<ToName>().0 = entities;
        app.update();

        assert_eq!(
            app.world().resource::<Names>().0,
            expected,
            "每个载荷都要有自己的名字，不能落到兜底的 `action`"
        );
    }

    /// 认不出来的实体退回 `action`——**兜底只该是"新载荷还没取名"的临时状态**。
    #[test]
    fn an_unknown_action_falls_back_to_the_generic_word() {
        let (mut app, _) = naming_app();
        let orphan = app.world_mut().spawn_empty().id();
        app.world_mut().resource_mut::<ToName>().0 = vec![orphan];
        app.update();

        assert_eq!(app.world().resource::<Names>().0, vec!["action"]);
    }
}
