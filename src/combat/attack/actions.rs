//! 技能行动：载荷 + 工厂 + 声明 / 执行系统。
//!
//! 每个技能 = 一个载荷组件 + 一个行动工厂 + 一个执行器。执行器到点后生成的
//! 攻击实体（箭矢 / 横扫）走通用战斗流水线，时间线完全不参与。
//!
//! 执行器**自己收尾**：`now >= execute_at` 才动手，然后销毁行动实体、
//! 把行动者忙到效果真的发生为止（箭矢要忙到落地，否则箭会冻在半空）。
//!
//! **两种远程手段的分工**：火球（[`super::fireball`]）锁格 + 半径 AoE，
//! 箭矢（本文件的射击一族）是**单体狙击**——伤害略低、出手更快、追踪一个目标。
//! 玩家按 `SkillKind::Shoot` 那一格（或将来绑热键）走 [`declare_shoot_system`]，
//! 与火球各走各的声明系统（它们的落点语义不同：锁格 vs 追踪）。

use bevy::prelude::*;

use crate::combat::Faction;
use crate::combat::defense::Stamina;
use crate::combat::reaction::{Threatens, melee_arc_cells};
use crate::movement::Cell;
use crate::skills::AbilityId;
use crate::timeline::{
    ActionOf, ActionTiming, DecisionSlot, FirstReady, Focus, InputDriven, Intent, PendingFocus,
    ScheduledAction, Target,
};

use super::arrow::{ARROW_SPEED, arrow_scene};
use super::events::ShootCommand;
use super::melee::melee_scene;

/// 射击（箭矢）的节奏：出手慢、后摇长，但在手里的时候最怕被打断。
pub const ARROW_TIMING: ActionTiming = ActionTiming::new(0.30, 0.50, 2);
/// 近战的节奏：出手快、硬直长、抗打断中等。
pub const MELEE_TIMING: ActionTiming = ActionTiming::new(0.20, 0.35, 3);

/// 「声明攻击时要查的玩家」：身份 + 位置 + 阵营 + Focus + 决策槽。
///
/// 抽成类型别名是因为元组变长了（`Focus` 变成每单位一份的组件之后），
/// 直接写在签名里会触发 `clippy::type_complexity`。
type AttackerPlayer<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Cell,
        &'static Transform,
        &'static Faction,
        &'static mut Focus,
        &'static DecisionSlot,
    ),
    With<InputDriven>,
>;

/// 射击载荷：朝最近敌人放一支箭。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ShootAction;

/// 近战载荷：朝最近敌人横扫一次。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MeleeAction;

/// 撤销一次近战要付的精力：抡出去再收招，比火球轻（见
/// [`refund_melee_observer`]）。
pub const MELEE_CANCEL_PENALTY: u32 = 1;

/// 射击行动工厂。
pub fn shoot_action_scene(
    timing: ActionTiming,
    schedule: ScheduledAction,
    actor: Entity,
) -> impl Scene {
    // 对抗标签（能不能被打断 / 招架 / 格挡）跟着载荷一起挂在行动实体上
    let tags = super::abilities::SHOOT_ABILITY.combat;
    bsn! {
        template_value(tags)
        ActionOf({actor})
        ShootAction
        template_value(timing)
        template_value(schedule)
    }
}

/// 近战行动工厂：声明这一次横扫**威胁到的格**（正前方一格 + 左右各一格）。
///
/// 威胁格是玩家 / AI 反应系统的输入（见 [`crate::combat::reaction`]）：
/// 一条正在前摇、且扇形压到你的横扫，就是"看得见的一刀"。
pub fn melee_action_scene(
    from_cell: Cell,
    target_cell: Cell,
    timing: ActionTiming,
    schedule: ScheduledAction,
    actor: Entity,
) -> impl Scene {
    let threatens = Threatens {
        cells: melee_arc_cells(from_cell, target_cell),
    };
    bsn! {
        ActionOf({actor})
        MeleeAction
        template_value(threatens)
        template_value(timing)
        template_value(schedule)
    }
}

/// 撤销近战：抡出去再收招要付一点精力。
///
/// 退多少、收多少归**花钱的那个领域**：时间线只触发
/// [`ActionCancelled`](crate::timeline::ActionCancelled)，这里自己认载荷。
pub fn refund_melee_observer(
    cancelled: On<crate::timeline::ActionCancelled>,
    actions: Query<(), With<MeleeAction>>,
    mut units: Query<&mut Stamina>,
) {
    if actions.get(cancelled.entity).is_err() {
        return; // 被撤的不是近战
    }
    if let Ok(mut stamina) = units.get_mut(cancelled.actor) {
        stamina.try_spend(MELEE_CANCEL_PENALTY);
    }
}

/// 最近的敌对单位所在的格（没有敌人时返回 `None`）。
fn nearest_enemy_cell(
    units: &Query<(&Transform, &Faction)>,
    origin: Vec3,
    faction: Faction,
) -> Option<Cell> {
    units
        .iter()
        .filter(|(_, unit_faction)| **unit_faction != faction)
        .min_by(|(a, _), (b, _)| {
            a.translation
                .distance_squared(origin)
                .total_cmp(&b.translation.distance_squared(origin))
        })
        .map(|(transform, _)| Cell::from_world(transform.translation))
}

/// 声明射击：`ShootCommand` → 一条射击行动（弓，单体狙击）。
///
/// 与 [`declare_fireball_system`](super::fireball::declare_fireball_system) 同形：
/// **条件校验只有一份**（`can_cast`）、节奏从配置读、武器偏移经
/// [`weapon_timing`](crate::equipment::weapon_timing) 加上。
///
/// **不接目标格**：箭矢的落点是"射手到最近敌人的那条线"，
/// 由执行器在**执行那一帧**决定（见 [`shoot_action_executor_system`]）——
/// 声明与落地之间敌人还能走开，所以在声明时锁格与"追踪"的语义不符。
///
/// **精力在声明时扣吗**：不。箭矢当前 `cost = 0`（`config/actions.ron` 里可调），
/// 所以这里没有扣费这一步；哪天给它写上了消耗，就照火球那条路在声明时扣。
#[allow(clippy::too_many_arguments)]
pub fn declare_shoot_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    pending_focus: Res<PendingFocus>,
    mut shoots: MessageReader<ShootCommand>,
    mut blocked: MessageWriter<crate::timeline::ActionBlocked>,
    config: Option<Res<crate::config::ActionConfig>>,
    // 武器改动作节奏（只给偏移，见 `equipment::weapon_timing`）
    equipment: Query<&crate::equipment::EquipmentBonus>,
    mut players: AttackerPlayer<'_, '_>,
) {
    if shoots.read().last().is_none() {
        return;
    }
    let Some((player, _, _, _, mut focus, _)) = players.iter_mut().first_ready(&mut blocked) else {
        return; // 忙或没有玩家
    };
    let base = match config.as_deref() {
        Some(config) => config.shoot.timing(),
        None => ARROW_TIMING,
    };
    let timing = crate::equipment::weapon_timing(base, equipment.get(player).ok());
    let now = time.elapsed_secs();
    let schedule = ScheduledAction::with_focus(timing, now, &mut focus, pending_focus.wants());
    declare_shoot_at(&mut commands, player, timing, schedule);
}

/// 声明一次射击（只生成行动实体）：扣费与触发源由调用方负责。
pub fn declare_shoot_at(
    commands: &mut Commands,
    actor: Entity,
    timing: ActionTiming,
    schedule: ScheduledAction,
) -> Entity {
    let action = commands
        .spawn_scene(shoot_action_scene(timing, schedule, actor))
        .id();
    // 这个工厂只有 `schedule`：声明时刻就是 `execute_at − windup`（前摇的定义）
    let declared_at = schedule.execute_at - timing.windup;
    commands.entity(actor).insert(DecisionSlot::declared(
        Intent {
            ability: AbilityId::Shoot,
            // 单体射击：目标在**执行那一帧**才确定（敌人会走开），所以这里不锁格
            target: Target::None,
        },
        &timing,
        declared_at,
    ));
    action
}

/// 声明一次近战横扫（只生成行动实体）：扣费与触发源由调用方负责。
pub fn declare_melee_at(
    commands: &mut Commands,
    actor: Entity,
    from_cell: Cell,
    target_cell: Cell,
    timing: ActionTiming,
    schedule: ScheduledAction,
) -> Entity {
    let action = commands
        .spawn_scene(melee_action_scene(
            from_cell,
            target_cell,
            timing,
            schedule,
            actor,
        ))
        .id();
    // 这个工厂只有 `schedule`：声明时刻就是 `execute_at − windup`（前摇的定义）
    let declared_at = schedule.execute_at - timing.windup;
    commands.entity(actor).insert(DecisionSlot::declared(
        Intent {
            ability: AbilityId::Melee,
            target: Target::Cell(target_cell),
        },
        &timing,
        declared_at,
    ));
    action
}

/// 执行射击：到点后从行动者位置朝最近敌人放箭，随后收尾。
///
/// 忙到**箭落地**为止：箭速 12 m/s，后摇只有 0.50s，只按后摇恢复决策槽的话，
/// 玩家一空闲世界就冻住，超距的箭会停在半空（和火球修复前是同一个坑）。
pub fn shoot_action_executor_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    config: Option<Res<crate::config::ActionConfig>>,
    actions: Query<(
        Entity,
        &ActionTiming,
        &ScheduledAction,
        &ShootAction,
        &ActionOf,
    )>,
    units: Query<(&Transform, &Faction)>,
    equipment: Query<&crate::equipment::EquipmentBonus>,
) {
    let now = time.elapsed_secs();
    for (entity, timing, schedule, _, action_of) in &actions {
        if !schedule.due(now) {
            continue;
        }
        let actor = action_of.actor();
        let mut effect_delay = 0.0;
        if let Ok((transform, faction)) = units.get(actor) {
            let origin = transform.translation;
            let faction = *faction;
            if let Some(target) = nearest_enemy_cell(&units, origin, faction) {
                let destination = target.center();
                let to_target = Vec3::new(destination.x - origin.x, 0.0, destination.y - origin.z);
                let direction = to_target.normalize_or_zero();
                effect_delay = to_target.length() / ARROW_SPEED;
                // 武器加成在生成时算好：攻击实体自己不认识"装备"
                let damage = crate::equipment::weapon_damage(
                    super::arrow::arrow_damage(config.as_deref()),
                    equipment
                        .get(actor)
                        .map(|bonus| bonus.damage())
                        .unwrap_or(0),
                );
                commands.spawn_scene(arrow_scene(
                    origin + direction * 1.2,
                    direction,
                    faction,
                    damage,
                ));
            }
        }
        let recovery = DecisionSlot::recovering(timing, schedule, effect_delay);
        commands.entity(entity).despawn();
        if let Ok(mut actor_commands) = commands.get_entity(actor) {
            actor_commands.insert(recovery);
        }
    }
}

/// 执行近战：到点后在行动者前方生成一次性横扫，随后收尾。
pub fn melee_action_executor_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    config: Option<Res<crate::config::ActionConfig>>,
    actions: Query<(
        Entity,
        &ActionTiming,
        &ScheduledAction,
        &MeleeAction,
        &ActionOf,
    )>,
    units: Query<(&Transform, &Faction)>,
    equipment: Query<&crate::equipment::EquipmentBonus>,
) {
    let now = time.elapsed_secs();
    for (entity, timing, schedule, _, action_of) in &actions {
        if !schedule.due(now) {
            continue;
        }
        let actor = action_of.actor();
        if let Ok((transform, faction)) = units.get(actor) {
            let origin = transform.translation;
            let faction = *faction;
            if let Some(target) = nearest_enemy_cell(&units, origin, faction) {
                let destination = target.center();
                let direction = Vec3::new(destination.x - origin.x, 0.0, destination.y - origin.z)
                    .normalize_or_zero();
                // 武器加成在生成时算好：攻击实体自己不认识"装备"
                let damage = crate::equipment::weapon_damage(
                    super::melee::melee_damage(config.as_deref()),
                    equipment
                        .get(actor)
                        .map(|bonus| bonus.damage())
                        .unwrap_or(0),
                );
                commands.spawn_scene(melee_scene(
                    origin + direction * 0.6,
                    direction,
                    faction,
                    damage,
                ));
            }
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
    use crate::timeline::ActionCancelled;

    /// 撤销近战要付 1 点收招费：规则住在花钱的领域，时间线只负责广播。
    #[test]
    fn cancelling_a_melee_costs_one_stamina() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_observer(refund_melee_observer);
        let actor = app.world_mut().spawn(Stamina::new(3)).id();
        let melee = app.world_mut().spawn(MeleeAction).id();

        app.world_mut().trigger(ActionCancelled {
            entity: melee,
            actor,
        });
        app.world_mut().flush();

        assert_eq!(
            app.world().get::<Stamina>(actor).unwrap().current,
            3 - MELEE_CANCEL_PENALTY,
            "抡出去再收招要付一点精力"
        );
    }

    /// 精力不够也拦不住改主意：只扣到 0。
    #[test]
    fn the_melee_cancel_cost_never_goes_below_zero() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_observer(refund_melee_observer);
        let actor = app.world_mut().spawn(Stamina::new(0)).id();
        let melee = app.world_mut().spawn(MeleeAction).id();

        app.world_mut().trigger(ActionCancelled {
            entity: melee,
            actor,
        });
        app.world_mut().flush();

        assert_eq!(app.world().get::<Stamina>(actor).unwrap().current, 0);
    }
}
