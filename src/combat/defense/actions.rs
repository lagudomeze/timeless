//! 防御域的声明 / 执行系统。
//!
//! - 翻滚载荷的工厂在 [`crate::movement`]（退一格与移动是同一个原语），
//!   本模块只负责**声明**与**落地**；
//! - 招架只绑实体，不需要位移，因此载荷与执行器都住在这里；
//! - 两个执行器都自己收尾：销毁行动实体 + 把行动者推进 `Recovery`（时间线不集中收尾）。

use bevy::prelude::*;

use crate::combat::Faction;
use crate::combat::attack::abilities::PARRY_ABILITY;
use crate::movement::abilities::ROLL_ABILITY;
use crate::movement::{Cell, ROLL_TIMING, Velocity, ground_direction, step_from_axis};
use crate::skills::{AbilityId, can_cast};
use crate::timeline::{
    ActionOf, ActionTiming, DecisionSlot, FirstReady, Focus, InputDriven, Intent, PendingFocus,
    ScheduledAction, Target,
};

use super::components::{ParryAction, Parrying};
use super::events::{ParryCommand, RollCommand};
use super::stamina::Stamina;

/// 翻滚消耗的精力。
pub const ROLL_COST: u32 = 1;
/// 招架消耗的精力。
pub const PARRY_COST: u32 = 1;
/// 翻滚的无敌帧时长（虚拟秒）。
pub const DODGE_SECS: f32 = 0.5;
/// 招架标记的兜底存活时长（虚拟秒）：目标攻击被销毁后不等它自然过期。
pub const PARRY_SECS: f32 = 0.5;
/// 招架的节奏：抬手一挡，姿态比翻滚稳一点。
pub const PARRY_TIMING: ActionTiming = ActionTiming::new(0.05, 0.25, 2);
/// 翻滚的位移速度（世界单位 / 秒）：比走路快，但仍然是「退一格」。
pub const ROLL_SPEED: f32 = 8.0;

/// 翻滚方向：远离最近的威胁（`threats` 给「位置 + 阵营」），没有威胁就不动。
///
/// 纯函数：玩家与 AI 共用同一份"往哪滚"的规则，谁触发只在调用方区分。
pub fn roll_step(
    origin: Vec3,
    faction: Faction,
    threats: impl Iterator<Item = (Vec3, Faction)>,
) -> (i32, i32) {
    let threat = threats
        .filter(|(_, other)| *other != faction)
        .map(|(position, _)| position)
        .min_by(|a, b| {
            a.distance_squared(origin)
                .total_cmp(&b.distance_squared(origin))
        });
    let Some(threat) = threat else {
        return (0, 0);
    };
    let away = origin - threat;
    step_from_axis(Vec2::new(away.x, away.z).normalize_or_zero())
}

/// 声明一次翻滚（载荷实体）。
///
/// 玩家走输入消息、AI 直接调用它，产出的行动实体完全一样；
/// 区别只在"谁触发"，不在"行动长什么样"。
pub fn declare_roll(
    commands: &mut Commands,
    actor: Entity,
    from_cell: Cell,
    to_cell: Cell,
    timing: ActionTiming,
    schedule: ScheduledAction,
) -> Entity {
    let action = commands
        .spawn_scene(crate::movement::roll_action_scene(
            from_cell, to_cell, timing, schedule, actor,
        ))
        .id();
    // 这个工厂只有 `schedule`，没有"现在几点"——但声明时刻就是
    // `execute_at − windup`（前摇的定义），所以不必再传一个 now 进来。
    let declared_at = schedule.execute_at - timing.windup;
    commands.entity(actor).insert(DecisionSlot::declared(
        Intent {
            ability: AbilityId::Roll,
            target: Target::Cell(to_cell),
        },
        &timing,
        declared_at,
    ));
    action
}

/// 声明翻滚（PC 路径）：`RollCommand` → 远离最近威胁退一格 + 无敌帧。
///
/// **只有 PC 靠按键决策**：`RollCommand` 是玩家输入消息，AI 不经它
/// （AI 的 `Intent::Dodge` 直接调 [`declare_roll`]）。
#[allow(clippy::too_many_arguments)]
/// 「翻滚时要查的玩家」：身份 + 位置 + 精力 + 位姿 + Focus + 决策槽。
///
/// 抽成别名是因为元组变长了（`Focus` 变成每单位一份的组件之后）。
type RollerPlayer<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Cell,
        &'static Stamina,
        &'static Transform,
        &'static mut Focus,
        &'static DecisionSlot,
    ),
    With<InputDriven>,
>;

#[allow(clippy::too_many_arguments)]
pub fn declare_roll_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    pending_focus: Res<PendingFocus>,
    mut requests: MessageReader<RollCommand>,
    mut blocked: MessageWriter<crate::timeline::ActionBlocked>,
    config: Option<Res<crate::config::ActionConfig>>,
    mut rollers: RollerPlayer<'_, '_>,
    units: Query<(&Transform, &Faction)>,
) {
    if requests.read().last().is_none() {
        return;
    }
    let Some((entity, cell, stamina, transform, mut focus, _)) =
        rollers.iter_mut().first_ready(&mut blocked)
    else {
        return;
    };
    // 翻滚的节奏 / 花费都从配置来（缺省 = 常量）
    let (timing, cost) = match config.as_deref() {
        Some(config) => (config.roll.timing(), config.roll.cost),
        None => (ROLL_TIMING, ROLL_COST),
    };
    let def = crate::skills::AbilityDef {
        timing,
        cost: crate::skills::ResourceCost::Energy(cost),
        ..ROLL_ABILITY
    };
    if let Err(reason) = can_cast(&def, crate::skills::Pools::new(stamina.current, 0)) {
        blocked.write(crate::timeline::ActionBlocked { reason });
        return;
    }

    let now = time.elapsed_secs();
    let (dx, dz) = roll_step(
        transform.translation,
        Faction::Player,
        units
            .iter()
            .map(|(other, faction)| (other.translation, *faction)),
    );
    let schedule = ScheduledAction::with_focus(timing, now, &mut focus, pending_focus.wants());
    declare_roll(
        &mut commands,
        entity,
        *cell,
        Cell::new(cell.x + dx, cell.z + dz),
        timing,
        schedule,
    );
}

/// 执行翻滚：朝目标格设速度 + 请求「到位时挂无敌帧」 + 扣精力。
///
/// **不直接改 `Cell`**：格子由 [`crate::movement::move_entities_system`] 在真正
/// 到达格中心时更新。否则决策层坐标会立刻跳到目标格，而世界坐标还没动
/// （表现为「滚了但人没动」）。
///
/// 收尾：销毁行动实体 + 把行动者忙到**滚到位**为止（`距离 / 速度`）。
pub fn roll_executor_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    actions: Query<(
        Entity,
        &ActionTiming,
        &crate::movement::RollAction,
        &ScheduledAction,
        &ActionOf,
    )>,
    mut actors: Query<(&mut Velocity, &mut Stamina, &Transform), With<Cell>>,
) {
    let now = time.elapsed_secs();
    for (entity, timing, roll, schedule, action_of) in &actions {
        if !schedule.due(now) {
            continue;
        }
        let actor = action_of.actor();
        let mut effect_delay = 0.0;
        if let Ok((mut velocity, mut stamina, transform)) = actors.get_mut(actor) {
            stamina.try_spend(ROLL_COST);
            let to_goal = roll.to_cell.center() - transform.translation.xz();
            velocity.0 = ground_direction(to_goal) * ROLL_SPEED;
            effect_delay = to_goal.length() / ROLL_SPEED;
            commands.entity(actor).insert((
                crate::movement::MoveGoal { cell: roll.to_cell },
                crate::movement::DodgingOnArrival {
                    expires_at: now + DODGE_SECS,
                },
            ));
        }
        let recovery = DecisionSlot::recovering(timing, schedule, effect_delay);
        commands.entity(entity).despawn();
        if let Ok(mut actor_commands) = commands.get_entity(actor) {
            actor_commands.insert(recovery);
        }
    }
}

/// 声明招架：`ParryCommand` → 挡下**正打向玩家的那一次攻击**。
///
/// 找不到威胁（没有挂在自己身上的 `CollisionTarget`）就不消耗精力、不占用决策槽——
/// 招架是反应，不该因为「空气招架」而白白失去一次行动机会。
#[allow(clippy::too_many_arguments)]
pub fn declare_parry_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    pending_focus: Res<PendingFocus>,
    mut requests: MessageReader<ParryCommand>,
    mut blocked: MessageWriter<crate::timeline::ActionBlocked>,
    config: Option<Res<crate::config::ActionConfig>>,
    mut players: Query<(Entity, &Stamina, &mut Focus, &DecisionSlot), With<InputDriven>>,
    attacks: Query<(Entity, &crate::combat::targeting::CollisionTarget)>,
) {
    if requests.read().last().is_none() {
        return;
    }
    let Some((player, stamina, mut focus, _)) = players.iter_mut().first_ready(&mut blocked) else {
        return;
    };
    let (timing, cost) = match config.as_deref() {
        Some(config) => (config.parry.timing(), config.parry.cost),
        None => (PARRY_TIMING, PARRY_COST),
    };
    let def = crate::skills::AbilityDef {
        timing,
        cost: crate::skills::ResourceCost::Energy(cost),
        ..PARRY_ABILITY
    };
    if let Err(reason) = can_cast(&def, crate::skills::Pools::new(stamina.current, 0)) {
        info!("招架失败：{reason:?}");
        blocked.write(crate::timeline::ActionBlocked { reason });
        return;
    }
    // 威胁 = 这次攻击的目标正是玩家自己
    let Some(target_attack) = attacks
        .iter()
        .find(|(_, marker)| marker.0 == player)
        .map(|(attack, _)| attack)
    else {
        return; // 没有威胁：这次输入什么也不做
    };

    let now = time.elapsed_secs();
    let schedule = ScheduledAction::with_focus(timing, now, &mut focus, pending_focus.wants());
    // `now` 用于上面的排期与下面的槽（声明即排期：`until = now + total`）
    commands.spawn_scene(crate::combat::defense::parry_action_scene(
        target_attack,
        timing,
        schedule,
        player,
    ));
    commands.entity(player).insert(DecisionSlot::declared(
        Intent {
            ability: AbilityId::Parry,
            target: Target::Entity(target_attack),
        },
        &PARRY_TIMING,
        now,
    ));
}

/// 执行招架：给行动者挂 [`Parrying`]，绑定被挡的那次攻击。
pub fn parry_executor_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    actions: Query<(
        Entity,
        &ActionTiming,
        &ParryAction,
        &ScheduledAction,
        &ActionOf,
    )>,
    mut actors: Query<&mut Stamina>,
) {
    let now = time.elapsed_secs();
    for (entity, timing, parry, schedule, action_of) in &actions {
        if !schedule.due(now) {
            continue;
        }
        let actor = action_of.actor();
        if let Ok(mut stamina) = actors.get_mut(actor) {
            #[allow(clippy::let_underscore_untyped)]
            let _ = PARRY_COST;
            stamina.try_spend(PARRY_COST);
            commands.entity(actor).insert(Parrying {
                target_attack: parry.target_attack,
                expires_at: now + PARRY_SECS,
            });
        }
        let recovery = DecisionSlot::recovering(timing, schedule, 0.0);
        commands.entity(entity).despawn();
        if let Ok(mut actor_commands) = commands.get_entity(actor) {
            actor_commands.insert(recovery);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movement::RollAction;

    /// PC 的按键**只作用于 PC**：就绪的敌人不会被 `RollCommand` 一起带着滚。
    #[test]
    fn the_players_roll_command_only_moves_the_player() {
        let mut app = crate::test_support::headless_app();
        app.update(); // Startup：组装玩家 + 敌人

        let (player, enemy) = {
            let mut query = app.world_mut().query::<(Entity, &Faction)>();
            let units: Vec<(Entity, Faction)> = query
                .iter(app.world())
                .map(|(entity, faction)| (entity, *faction))
                .collect();
            let find = |wanted: Faction| {
                units
                    .iter()
                    .find(|(_, faction)| *faction == wanted)
                    .map(|(entity, _)| *entity)
                    .expect("应当有单位")
            };
            (find(Faction::Player), find(Faction::Enemy))
        };

        app.world_mut().write_message(RollCommand);
        app.update();

        let rolls: Vec<Entity> = app
            .world_mut()
            .query_filtered::<Entity, With<RollAction>>()
            .iter(app.world())
            .collect();
        assert_eq!(rolls.len(), 1, "一次按键只该产生一条翻滚");
        let actor = app
            .world()
            .get::<ActionOf>(rolls[0])
            .expect("行动实体应当记得自己归谁")
            .actor();
        assert_eq!(actor, player, "翻滚必须挂在玩家身上");
        assert_ne!(actor, enemy, "敌人的决策槽不该被玩家的按键消耗");
    }
}
