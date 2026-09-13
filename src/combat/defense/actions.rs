//! 防御域的声明 / 执行系统与过期清理。
//!
//! - 翻滚载荷的工厂在 [`crate::movement`]（退一格与移动是同一个原语），
//!   本模块只负责**声明**与**落地**；
//! - 招架只绑实体，不需要位移，因此载荷与执行器都住在这里；
//! - 过期清理统一在这里，两个标记的语义放在一起看。

use bevy::prelude::*;

use crate::combat::Faction;
use crate::movement::{Cell, Velocity, ground_direction, step_from_axis};
use crate::timeline::{Declared, Ready, Timeline, begin_action};

use super::components::{Dodging, ParryAction, Parrying};
use super::events::{ParryCommand, RollCommand};
use super::stamina::Stamina;

/// 翻滚消耗的精力。
pub const ROLL_COST: u32 = 1;
/// 招架消耗的精力。
pub const PARRY_COST: u32 = 1;
/// 翻滚的无敌帧时长（虚拟秒）。
pub const DODGE_SECS: f32 = 0.5;
/// 翻滚标记的兜底存活时长（虚拟秒）：目标攻击被销毁后不等它自然过期。
pub const PARRY_SECS: f32 = 0.5;
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

/// 声明一次翻滚（载荷实体 + [`begin_action`]）——**行动是统一实体**的落点。
///
/// 玩家走输入消息、AI 直接调用它，产出的行动实体完全一样；
/// 区别只在"谁触发"，不在"行动长什么样"。
pub fn declare_roll(
    commands: &mut Commands,
    timeline: &mut Timeline,
    actor: Entity,
    from_cell: Cell,
    to_cell: Cell,
    now: f32,
) -> Entity {
    let draft = commands
        .spawn_scene(crate::movement::roll_action_scene(
            actor, from_cell, to_cell, now,
        ))
        .id();
    begin_action(commands, timeline, actor, draft);
    draft
}

/// 声明翻滚（PC 路径）：`RollCommand` → 远离最近威胁退一格 + 无敌帧。
///
/// **只有 PC 靠按键决策**：`RollCommand` 是玩家输入消息，AI 不经它
/// （AI 的 `Intent::Dodge` 直接调 [`declare_roll`]）。
pub fn declare_roll_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    mut timeline: ResMut<Timeline>,
    mut requests: MessageReader<RollCommand>,
    mut blocked: MessageWriter<crate::timeline::ActionBlocked>,
    mut rollers: Query<(Entity, &Cell, &Stamina, &Transform, &Faction), With<Ready>>,
    units: Query<(&Transform, &Faction)>,
) {
    if requests.read().last().is_none() {
        return;
    }
    // **必须按阵营挑玩家**：就绪的单位里也有敌人，不筛就会把玩家的按键挂到敌人身上。
    let Some((entity, cell, stamina, transform, _)) = rollers
        .iter_mut()
        .find(|(_, _, _, _, faction)| **faction == Faction::Player)
    else {
        blocked.write(crate::timeline::ActionBlocked::BUSY);
        return;
    };
    if !stamina.can_afford(ROLL_COST) {
        blocked.write(crate::timeline::ActionBlocked::NO_ENERGY);
        return;
    }

    let (dx, dz) = roll_step(
        transform.translation,
        Faction::Player,
        units
            .iter()
            .map(|(other, faction)| (other.translation, *faction)),
    );
    declare_roll(
        &mut commands,
        &mut timeline,
        entity,
        *cell,
        Cell::new(cell.x + dx, cell.z + dz),
        now.elapsed_secs(),
    );
}

/// 执行翻滚：朝目标格设速度 + 请求「到位时挂无敌帧」 + 扣精力。
///
/// **不直接改 `Cell`**：格子由 [`crate::movement::move_entities_system`] 在真正
/// 到达格中心时更新。否则决策层坐标会立刻跳到目标格，而世界坐标还没动
/// （表现为「滚了但人没动」）。
pub fn roll_executor_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    actions: Query<
        (
            Entity,
            &crate::movement::RollAction,
            &crate::timeline::ScheduledAction,
        ),
        With<crate::timeline::Committed>,
    >,
    mut actors: Query<(&mut Velocity, &mut Stamina, &Transform), With<Cell>>,
) {
    let now = now.elapsed_secs();
    for (entity, roll, schedule) in &actions {
        if let Ok((mut velocity, mut stamina, transform)) = actors.get_mut(schedule.actor) {
            stamina.try_spend(ROLL_COST);
            let target = roll.to_cell.center();
            velocity.0 = ground_direction(target - transform.translation.xz()) * ROLL_SPEED;
            crate::timeline::insert_on_actor(
                &mut commands,
                schedule.actor,
                (
                    crate::movement::MoveGoal { cell: roll.to_cell },
                    crate::movement::DodgingOnArrival {
                        expires_at: now + DODGE_SECS,
                    },
                ),
            );
        }
        crate::timeline::end_action(&mut commands, entity, schedule.actor, schedule, now);
    }
}

/// 声明招架：`ParryCommand` → 挡下当前正在前摇的那次攻击。
///
/// 找不到威胁（敌人没有待执行的攻击）就不消耗精力、不占用这次决策——
/// 招架是反应，不该因为「空气招架」而白白失去一次行动机会。
pub fn declare_parry_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    mut timeline: ResMut<Timeline>,
    mut requests: MessageReader<ParryCommand>,
    mut blocked: MessageWriter<crate::timeline::ActionBlocked>,
    mut players: Query<(Entity, &Stamina, &Faction), With<Ready>>,
    threats: Query<&crate::timeline::ScheduledAction, With<Declared>>,
) {
    if requests.read().last().is_none() {
        return;
    }
    // **必须按阵营挑玩家**：就绪的单位里也有敌人，`single_mut()` 会抓错人
    // （两个都就绪时还会直接失败 —— 表现为按 V 什么也没发生）。
    // `ParryCommand` 只有玩家写，所以找不到就绪的玩家就等于"这次按键被拒"。
    let Some((player, stamina, _)) = players
        .iter_mut()
        .find(|(_, _, faction)| **faction == Faction::Player)
    else {
        blocked.write(crate::timeline::ActionBlocked::BUSY);
        return;
    };
    if !stamina.can_afford(PARRY_COST) {
        info!("招架失败：精力不足");
        blocked.write(crate::timeline::ActionBlocked::NO_ENERGY);
        return;
    }
    // 威胁 = 任何「还在草案里」的动作实体（玩家与敌人共用一条声明流程）
    let Some(target_attack) = threats.iter().map(|s| s.actor).next() else {
        return; // 没有威胁：这次输入什么也不做
    };

    let draft = commands
        .spawn_scene(crate::combat::defense::parry_action_scene(
            player,
            target_attack,
            now.elapsed_secs(),
        ))
        .id();
    begin_action(&mut commands, &mut timeline, player, draft);
}

/// 执行招架：给行动者挂 [`Parrying`]，绑定被挡的那次攻击。
pub fn parry_executor_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    actions: Query<
        (Entity, &ParryAction, &crate::timeline::ScheduledAction),
        With<crate::timeline::Committed>,
    >,
    mut actors: Query<&mut Stamina>,
) {
    let now = now.elapsed_secs();
    for (entity, parry, schedule) in &actions {
        if let Ok(mut stamina) = actors.get_mut(schedule.actor) {
            stamina.try_spend(PARRY_COST);
            crate::timeline::insert_on_actor(
                &mut commands,
                schedule.actor,
                Parrying {
                    target_attack: parry.target_attack,
                    expires_at: now + PARRY_SECS,
                },
            );
        }
        crate::timeline::end_action(&mut commands, entity, schedule.actor, schedule, now);
    }
}

/// 过期清理：[`Dodging`] 到点移除；[`Parrying`] 到点或绑定的攻击消失即移除。
pub fn expire_defense_markers_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    attacks: Query<(), With<crate::timeline::ScheduledAction>>,
    dodging: Query<(Entity, &Dodging)>,
    parrying: Query<(Entity, &Parrying)>,
) {
    let now = now.elapsed_secs();
    for (entity, dodging) in &dodging {
        if now >= dodging.expires_at {
            commands.entity(entity).remove::<Dodging>();
        }
    }
    for (entity, parrying) in &parrying {
        // 绑定的攻击实体已经不存在（打空了 / 被销毁）→ 标记失去意义
        let target_gone = attacks.get(parrying.target_attack).is_err();
        if now >= parrying.expires_at || target_gone {
            commands.entity(entity).remove::<Parrying>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movement::RollAction;

    /// PC 的按键**只作用于 PC**：就绪的敌人不会被 `RollCommand` 一起带着滚。
    ///
    /// 无回合模型里"谁能决策"由各自 `Ready` 决定，但**触发源**只有玩家：AI 不走输入
    /// 消息（见 `ai::systems::enemy_declare_system`）。
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
            .get::<crate::timeline::ScheduledAction>(rolls[0])
            .expect("行动实体应当带调度数据")
            .actor;
        assert_eq!(actor, player, "翻滚必须挂在玩家身上");
        assert_ne!(actor, enemy, "敌人的 Ready 不该被玩家的按键消耗");
    }
}
