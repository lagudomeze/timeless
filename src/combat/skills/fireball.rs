//! 火球：锁目标格 → 自由飞行 → 到达后按**真实距离**结算 AoE。
//!
//! 与箭矢的区别（两种投射物，两种机制）：
//!
//! | | 箭矢 | 火球 |
//! | :--- | :--- | :--- |
//! | 命中方式 | 碰撞（`CollisionTarget` + `HitRadius`） | 到达目标格 → 半径内全体 |
//! | 落点 | 追踪最近敌人 | **声明时锁定的格**（敌人可以走开） |
//! | 判定距离 | 碰撞半径 | 真实距离 ≤ `FIREBALL_RADIUS` |
//!
//! 「锁格 + 真实距离」正是决策按格、结算按真实距离的直接体现：点的是格，炸的是米。

use bevy::prelude::*;

use crate::combat::Faction;
use crate::combat::attributes::{AttackFrame, HitRadius, InterruptPower, PhysicalDamage};
use crate::combat::lifecycle::Projectile;
use crate::combat::reaction::{TargetCell, Threatens, trajectory_cells};
use crate::movement::{Cell, Velocity};
use crate::timeline::{
    ActionOf, ActionTiming, DecisionSlot, FirstReady, Focus, FocusIntent, InputDriven,
    ScheduledAction,
};

use super::actions::MELEE_TIMING;
use super::events::{FireCommand, MeleeCommand};

/// 火球投射物的飞行参数与爆炸参数（目标格住在 [`TargetCell`] 上）。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Fireball {
    /// 飞行速度（世界单位 / 秒）
    pub speed: f32,
    /// 爆炸伤害
    pub amount: i32,
    /// 爆炸半径（世界单位，按真实距离判定）
    pub radius: f32,
}

impl Default for Fireball {
    fn default() -> Self {
        Self {
            speed: FIREBALL_SPEED,
            amount: FIREBALL_DAMAGE,
            radius: FIREBALL_RADIUS,
        }
    }
}

/// 火球行动载荷：`FireCommand` 的产物，锁着目标格。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FireballAction {
    /// 锁定的目标格（发射时用）
    pub target_cell: Cell,
}

/// 火球飞行速度（世界单位 / 秒）：一格 2.0 → 约 0.25s 飞一格。
pub const FIREBALL_SPEED: f32 = 8.0;
/// 出手高度（世界单位）：从脚底往上抬一点扔，避免火球贴地穿模。
pub const SHOOT_HEIGHT: f32 = 0.9;
/// 火球爆炸伤害。
pub const FIREBALL_DAMAGE: i32 = 12;
/// 爆炸半径（世界单位）：1.5 格。
pub const FIREBALL_RADIUS: f32 = 3.0;
/// 火球的节奏：出手慢、后摇长、威力大。
pub const FIREBALL_TIMING: ActionTiming = ActionTiming::new(0.30, 0.50, 2);
/// 火球消耗的精力（比翻滚贵，构成资源取舍）。
pub const FIREBALL_COST: u32 = 2;
/// 火球的打断力度：出手重，但正在前摇时最怕被打断（见 [`FIREBALL_TIMING`]）。
pub const FIREBALL_POWER: i32 = 2;

/// 火球行动工厂：载荷 + 威胁声明（飞过的格 + 落点）+ 调度数据。
pub fn fireball_action_scene(
    from_cell: Cell,
    target_cell: Cell,
    timing: ActionTiming,
    schedule: ScheduledAction,
    actor: Entity,
) -> impl Scene {
    let threatens = Threatens {
        cells: trajectory_cells(from_cell, target_cell),
    };
    bsn! {
        ActionOf({actor})
        FireballAction { target_cell: {target_cell} }
        template_value(threatens)
        template_value(timing)
        template_value(schedule)
    }
}

/// 撤销火球：声明时扣掉的精力原样退回，再收同样多的手续费。
///
/// 「退 2 又收 2」是故意的：撤销这一步本身要有分量，否则大招可以随手撤掉重来。
/// 规则归**花钱的那个领域**——时间线只触发
/// [`ActionCancelled`](crate::timeline::ActionCancelled)，这里自己认载荷。
pub fn refund_fireball_observer(
    cancelled: On<crate::timeline::ActionCancelled>,
    actions: Query<(), With<FireballAction>>,
    mut units: Query<&mut crate::combat::defense::Stamina>,
) {
    if actions.get(cancelled.entity).is_err() {
        return; // 被撤的不是火球
    }
    let Ok(mut stamina) = units.get_mut(cancelled.actor) else {
        return; // 行动者可能已经阵亡
    };
    stamina.regen(FIREBALL_COST);
    stamina.try_spend(FIREBALL_COST);
}

/// 火球实体工厂：朝目标格飞行的投射物（到达后由到达系统广播）。
///
/// 挂 [`TargetCell`]：飞行中的它同样构成威胁（反应系统据此冻结世界，
/// 玩家还有机会躲开或者抢先把它打掉）。
pub fn fireball_scene(origin: Vec3, target_cell: Cell, faction: Faction) -> impl Scene {
    let target = target_cell.center();
    // 落到目标格中心正上方一点，避免贴地穿模
    let destination = Vec3::new(target.x, origin.y, target.y);
    let direction = (destination - origin).normalize_or_zero();
    let speed = if origin.distance(destination) > f32::EPSILON {
        FIREBALL_SPEED
    } else {
        0.0
    };
    let rotation = if direction == Vec3::ZERO {
        Quat::IDENTITY
    } else {
        Quat::from_rotation_arc(Vec3::Z, direction)
    };
    bsn! {
        template_value(faction)
        template_value(Velocity(direction * speed))
        template_value(Fireball::default())
        TargetCell(target_cell)
        Projectile { max_hits: 0, current_hits: 0, finished: false }
        template_value(PhysicalDamage(FIREBALL_DAMAGE))
        template_value(AttackFrame(7))
        template_value(InterruptPower(FIREBALL_POWER))
        HitRadius(0.35)
        Transform {
            translation: {origin},
            rotation: {rotation},
        }
        Mesh3d(asset_value(Sphere::new(0.35)))
        MeshMaterial3d<StandardMaterial>(asset_value(StandardMaterial {
            base_color: Color::srgb(1.0, 0.45, 0.15),
            emissive: LinearRgba::rgb(6.0, 1.2, 0.2),
            unlit: true,
            ..default()
        }))
    }
}

/// 声明火球：`FireCommand` → 朝目标格放一发（锁格），并扣精力。
///
/// 精力在声明时就扣（比执行时扣更难被「先声明后没钱」钻空子），
/// 且精力不足时不占用这次决策。
type FireballPlayer<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Cell,
        &'static mut crate::combat::defense::Stamina,
        &'static Transform,
        &'static Faction,
        &'static DecisionSlot,
    ),
    With<InputDriven>,
>;

/// 见 [`FireballPlayer`]。
#[allow(clippy::too_many_arguments)]
pub fn declare_fireball_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    mut focus: ResMut<Focus>,
    intent: Res<FocusIntent>,
    mut fires: MessageReader<FireCommand>,
    mut blocked: MessageWriter<crate::timeline::ActionBlocked>,
    mut players: FireballPlayer<'_, '_>,
    units: Query<(&Transform, &Faction)>,
) {
    let Some(request) = fires.read().last().copied() else {
        return;
    };
    let Some((player, cell, mut stamina, transform, faction, _)) =
        players.iter_mut().first_ready(&mut blocked)
    else {
        return; // 忙或没有玩家
    };
    if !stamina.can_afford(FIREBALL_COST) {
        blocked.write(crate::timeline::ActionBlocked::NO_ENERGY);
        info!("火球失败：精力不足");
        return;
    }

    // 锁格：鼠标给了目标格就打那一格；键盘没给就打最近敌对单位所在的格
    let origin = transform.translation;
    let faction = *faction;
    let target_cell = request.target_cell.unwrap_or_else(|| {
        units
            .iter()
            .filter(|(_, other)| **other != faction)
            .min_by(|(a, _), (b, _)| {
                a.translation
                    .distance_squared(origin)
                    .total_cmp(&b.translation.distance_squared(origin))
            })
            .map(|(target, _)| Cell::from_world(target.translation))
            .unwrap_or(*cell)
    });

    stamina.try_spend(FIREBALL_COST);
    let now = time.elapsed_secs();
    let schedule = ScheduledAction::with_focus(FIREBALL_TIMING, now, &mut focus, intent.0);
    declare_fireball_at(
        &mut commands,
        player,
        *cell,
        target_cell,
        FIREBALL_TIMING,
        schedule,
    );
}

/// 声明近战：`MeleeCommand` → 技能域的横扫行动（不消耗精力）。
#[allow(clippy::too_many_arguments)]
pub fn declare_melee_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    mut focus: ResMut<Focus>,
    intent: Res<FocusIntent>,
    mut melees: MessageReader<MeleeCommand>,
    mut blocked: MessageWriter<crate::timeline::ActionBlocked>,
    players: Query<(Entity, &Cell, &Transform, &Faction, &DecisionSlot), With<InputDriven>>,
    units: Query<(&Transform, &Faction)>,
) {
    if melees.read().last().is_none() {
        return;
    }
    let Some((player, cell, transform, faction, _)) = players.iter().first_ready(&mut blocked)
    else {
        return;
    };
    let target_cell = units
        .iter()
        .filter(|(_, other)| other != &faction)
        .min_by(|(a, _), (b, _)| {
            a.translation
                .distance_squared(transform.translation)
                .total_cmp(&b.translation.distance_squared(transform.translation))
        })
        .map(|(target, _)| Cell::from_world(target.translation))
        .unwrap_or(*cell);
    let now = time.elapsed_secs();
    let schedule = ScheduledAction::with_focus(MELEE_TIMING, now, &mut focus, intent.0);
    super::actions::declare_melee_at(
        &mut commands,
        player,
        *cell,
        target_cell,
        MELEE_TIMING,
        schedule,
    );
}

/// 声明一次火球（只生成**行动实体**），**不检查精力**。
///
/// 玩家路径（[`declare_fireball_system`]）负责扣精力；AI 路径直接调用它。
/// 发射点与发射者都取自**执行那一帧**的世界状态
/// （见 [`fireball_action_executor_system`]）：声明与落地之间这一发还能被撤销，
/// 在声明时就生成投射物会留下撤不干净的半空火球。
pub fn declare_fireball_at(
    commands: &mut Commands,
    actor: Entity,
    from_cell: Cell,
    target_cell: Cell,
    timing: ActionTiming,
    schedule: ScheduledAction,
) -> Entity {
    let action = commands
        .spawn_scene(fireball_action_scene(
            from_cell,
            target_cell,
            timing,
            schedule,
            actor,
        ))
        .id();
    commands.entity(actor).insert(DecisionSlot::Windup);
    action
}

/// 执行火球行动：发射投射物，然后**忙到火球落地**，之后才是后摇。
///
/// 火球的效果发生在落地那一刻，而飞行时间随距离变长；只忙一个后摇的话，
/// 远程火球会在玩家恢复决策槽时被冻在半空（实机表现："按了技能没放出去，
/// 但精力已经扣了"）。
pub fn fireball_action_executor_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    actions: Query<(
        Entity,
        &ActionTiming,
        &ScheduledAction,
        &FireballAction,
        &ActionOf,
    )>,
    actors: Query<(&Transform, &Faction)>,
) {
    let now = time.elapsed_secs();
    for (entity, timing, schedule, action, action_of) in &actions {
        if !schedule.due(now) {
            continue;
        }
        let actor = action_of.actor();
        let mut effect_delay = 0.0;
        // 发射：从行动者**当前位置**朝锁定的格扔出投射物
        if let Ok((transform, faction)) = actors.get(actor) {
            let origin = Vec3::new(
                transform.translation.x,
                transform.translation.y + SHOOT_HEIGHT,
                transform.translation.z,
            );
            effect_delay = flight_time(origin, action.target_cell);
            commands.spawn_scene(fireball_scene(origin, action.target_cell, *faction));
        }
        let recovery = DecisionSlot::recovering(timing, now, effect_delay);
        commands.entity(entity).despawn();
        if let Ok(mut actor_commands) = commands.get_entity(actor) {
            actor_commands.insert(recovery);
        }
    }
}

/// 到达判定：飞抵目标格中心附近就广播 [`ProjectileArrived`] 并结束飞行。
///
/// 半径判定用**真实距离**（格中心到投射物位置），因此从斜角飞来的火球
/// 也在正确的位置炸。到达时摘掉 [`TargetCell`]：它不再是威胁了。
pub fn projectile_arrival_system(
    mut commands: Commands,
    mut arrived: MessageWriter<ProjectileArrived>,
    mut shells: Query<(
        Entity,
        &mut Transform,
        &mut Velocity,
        &Fireball,
        &TargetCell,
        &Faction,
    )>,
) {
    for (entity, mut transform, mut velocity, fireball, target_cell, faction) in &mut shells {
        let target = target_cell.0.center();
        let position = transform.translation.xz();
        if position.distance(target) > ARRIVAL_TOLERANCE {
            continue;
        }
        transform.translation.x = target.x;
        transform.translation.z = target.y;
        velocity.0 = Vec3::ZERO;
        arrived.write(ProjectileArrived {
            projectile: entity,
            cell: target_cell.0,
            origin: Vec3::new(target.x, transform.translation.y, target.y),
            faction: *faction,
            damage: fireball.amount,
            radius: fireball.radius,
        });
        commands
            .entity(entity)
            .remove::<TargetCell>()
            .remove::<Fireball>()
            .insert(Projectile {
                max_hits: 0,
                current_hits: 1,
                finished: true,
            });
    }
}

/// 落点容差（世界单位）：小于它就算「到了那一格」。
pub const ARRIVAL_TOLERANCE: f32 = 0.2;

/// 从 `origin` 平飞到目标格中心要多久（虚拟秒）。
///
/// 飞行时间必须算进「行动者忙到什么时候」：只按后摇恢复决策槽的话，
/// 玩家一空闲世界就冻住，火球会停在半空。
pub fn flight_time(origin: Vec3, target_cell: Cell) -> f32 {
    let target = target_cell.center();
    let destination = Vec3::new(target.x, origin.y, target.y);
    origin.distance(destination) / FIREBALL_SPEED
}

/// 爆炸落点消息（写：到达系统；消费：爆炸结算）。
#[derive(Message, Debug, Clone, Copy)]
pub struct ProjectileArrived {
    /// 投射物实体（爆炸后销毁）
    pub projectile: Entity,
    /// 落点格
    pub cell: Cell,
    /// 落点世界坐标（爆炸中心，用于真实距离判定）
    pub origin: Vec3,
    /// 投掷方阵营（只炸敌人）
    pub faction: Faction,
    /// 爆炸伤害
    pub damage: i32,
    /// 爆炸半径（世界单位）
    pub radius: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_target_cell_is_locked_at_declaration() {
        let target = Cell::new(3, -2);
        let threatens = Threatens {
            cells: trajectory_cells(Cell::new(0, 0), target),
        };
        assert_eq!(
            threatens.cells.last(),
            Some(&target),
            "落点在声明后不应该被追着敌人改"
        );
        assert_eq!(Fireball::default().radius, FIREBALL_RADIUS);
    }

    #[test]
    fn radius_is_expressed_in_world_units() {
        assert_eq!(
            FIREBALL_RADIUS,
            crate::movement::CELL_SIZE * 1.5,
            "1.5 格的世界距离"
        );
    }

    /// 撤销火球：退出手时扣的 2 点，再收 2 点手续费 → 净额不变。
    ///
    /// 退多少由**花钱的领域**决定：时间线只广播"这条行动被撤了"。
    #[test]
    fn cancelling_a_fireball_refunds_and_recharges_the_same_amount() {
        use crate::combat::defense::Stamina;
        use crate::timeline::ActionCancelled;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_observer(refund_fireball_observer);
        let actor = app.world_mut().spawn(Stamina::new(3)).id();
        // 声明那一刻已经扣掉了 FIREBALL_COST
        app.world_mut()
            .get_mut::<Stamina>(actor)
            .unwrap()
            .try_spend(FIREBALL_COST);
        let fireball = app.world_mut().spawn(FireballAction::default()).id();

        app.world_mut().trigger(ActionCancelled {
            entity: fireball,
            actor,
        });
        app.world_mut().flush();
        assert_eq!(
            app.world().get::<Stamina>(actor).unwrap().current,
            1,
            "退 2 收 2：撤销一次火球的净额不变"
        );

        // 被撤的不是火球（别的载荷）：一分不动
        let other = app.world_mut().spawn_empty().id();
        app.world_mut().trigger(ActionCancelled {
            entity: other,
            actor,
        });
        app.world_mut().flush();
        assert_eq!(
            app.world().get::<Stamina>(actor).unwrap().current,
            1,
            "别的行动被撤不该动火球的账"
        );
    }
}
