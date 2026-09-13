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
//! 「锁格 + 真实距离」正是决策按格、结算按真实距离的直接体现：
//! 点的是格，炸的是米。

use bevy::prelude::*;

use crate::combat::Faction;
use crate::combat::attributes::{AttackFrame, HitRadius, Impact, PhysicalDamage};
use crate::combat::lifecycle::Projectile;
use crate::movement::{Cell, Velocity};
use crate::timeline::{Declared, Ready, ScheduledAction, Timeline, begin_action, timing};

use super::events::{FireCommand, MeleeCommand};

/// 火球：锁定的目标格 + 飞行参数 + 爆炸参数。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Fireball {
    /// 落点格（声明时锁定，飞行途中不会改追）
    pub target_cell: Cell,
    /// 飞行速度（世界单位 / 秒）
    pub speed: f32,
    /// 爆炸伤害
    pub amount: f32,
    /// 爆炸半径（世界单位，按真实距离判定）
    pub radius: f32,
}

impl Default for Fireball {
    fn default() -> Self {
        Self {
            target_cell: Cell::default(),
            speed: FIREBALL_SPEED,
            amount: FIREBALL_DAMAGE,
            radius: FIREBALL_RADIUS,
        }
    }
}

/// 火球行动载荷：`FireCommand` 的产物。
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
pub const FIREBALL_DAMAGE: f32 = 12.0;
/// 爆炸半径（世界单位）：1.5 格。
pub const FIREBALL_RADIUS: f32 = 3.0;
/// 火球消耗的精力（比翻滚贵，构成资源取舍）。
pub const FIREBALL_COST: u32 = 2;

/// 火球行动工厂。
pub fn fireball_action_scene(actor: Entity, target_cell: Cell, now: f32) -> impl Scene {
    let schedule = ScheduledAction::declared_at(actor, timing::SHOOT, now);
    bsn! {
        FireballAction { target_cell: {target_cell} }
        template_value(schedule)
        // 取消代价 = 一次技能的钱：大招打断不能白打断
        template_value(crate::timeline::CancelCost(FIREBALL_COST))
        Declared
    }
}

/// 火球实体工厂：朝目标格飞行的投射物（到达后由到达系统广播）。
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
    let fireball = Fireball {
        target_cell,
        ..Fireball::default()
    };
    bsn! {
        template_value(faction)
        template_value(Velocity(direction * speed))
        template_value(fireball)
        Projectile { max_hits: 0, current_hits: 0, finished: false }
        template_value(PhysicalDamage(FIREBALL_DAMAGE))
        template_value(AttackFrame(7))
        template_value(Impact(2))
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

/// 声明火球：`FireCommand` → 朝最近敌人的**格子**放一发（锁格），并扣精力。
///
/// 精力在声明时就扣（比执行时扣更难被「先声明后没钱」钻空子），
/// 且精力不足时不占用这次决策。
pub fn declare_fireball_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    mut timeline: ResMut<Timeline>,
    mut fires: MessageReader<FireCommand>,
    mut blocked: MessageWriter<crate::timeline::ActionBlocked>,
    mut players: Query<
        (
            Entity,
            &Cell,
            &mut crate::combat::defense::Stamina,
            &Transform,
            &Faction,
        ),
        With<Ready>,
    >,
    units: Query<(&Transform, &Faction)>,
) {
    let Some(request) = fires.read().last().copied() else {
        return;
    };
    // 必须按阵营挑玩家：`single_mut()` 可能抓到敌人
    let player = players
        .iter_mut()
        .find(|(_, _, _, _, faction)| **faction == Faction::Player);
    let Some((player, cell, mut stamina, transform, faction)) = player else {
        blocked.write(crate::timeline::ActionBlocked::BUSY);
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
    let action = declare_fireball_at(
        &mut commands,
        &mut timeline,
        player,
        target_cell,
        now.elapsed_secs(),
    );
    // 记下这次花掉多少：撤销时要原样退回来
    commands
        .entity(action)
        .insert(crate::timeline::ActionCost(FIREBALL_COST));
}

/// 声明近战：`MeleeCommand` → 技能域的横扫行动（不消耗精力）。
pub fn declare_melee_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    mut timeline: ResMut<Timeline>,
    mut melees: MessageReader<MeleeCommand>,
    mut blocked: MessageWriter<crate::timeline::ActionBlocked>,
    players: Query<(Entity, &Faction), With<Ready>>,
) {
    if melees.read().last().is_none() {
        return;
    }
    let player = players
        .iter()
        .find(|(_, faction)| **faction == Faction::Player)
        .map(|(entity, _)| entity);
    let Some(player) = player else {
        blocked.write(crate::timeline::ActionBlocked::BUSY);
        return;
    };
    let draft = commands
        .spawn_scene(super::actions::melee_action_scene(
            player,
            now.elapsed_secs(),
        ))
        .id();
    begin_action(&mut commands, &mut timeline, player, draft);
}

/// 声明一次火球（只生成**行动实体**），**不检查精力**。
///
/// 玩家路径（[`declare_fireball_system`]）负责扣精力；AI 路径直接调用它
/// （敌人当前没有精力预算，见 [`TODO.md`](../../../TODO.md) 的「资源分线」）。
/// 发射点与发射者都取自**执行那一帧**的世界状态
/// （见 [`fireball_action_executor_system`]）：声明与落地之间这一发还能被撤销，
/// 在声明时就生成投射物会留下撤不干净的半空火球。
pub fn declare_fireball_at(
    commands: &mut Commands,
    timeline: &mut Timeline,
    actor: Entity,
    target_cell: Cell,
    now: f32,
) -> Entity {
    let draft = commands
        .spawn_scene(fireball_action_scene(actor, target_cell, now))
        .id();
    begin_action(commands, timeline, actor, draft);
    draft
}

/// 执行火球行动：发射投射物，然后**忙到火球落地**，之后才是后摇。
///
/// 和移动执行器同一个道理（`movement/actions.rs`）：忙到"效果真的发生"为止。
/// 火球的效果发生在落地那一刻，而飞行时间随距离变长；只忙一个后摇的话，
/// 远程火球会在玩家恢复 [`Ready`] 的那一帧被冻在半空。
pub fn fireball_action_executor_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    actions: Query<(Entity, &ScheduledAction, &FireballAction), With<crate::timeline::Committed>>,
    actors: Query<(&Transform, &Faction)>,
) {
    for (entity, schedule, action) in &actions {
        let executed_at = now.elapsed_secs();
        // 默认只忙一个后摇；发射出去之后按飞行时间顺延
        let mut busy_until = executed_at;
        // 发射：从行动者**当前位置**朝锁定的格扔出投射物
        if let Ok((transform, faction)) = actors.get(schedule.actor) {
            let origin = Vec3::new(
                transform.translation.x,
                transform.translation.y + SHOOT_HEIGHT,
                transform.translation.z,
            );
            busy_until = executed_at + flight_time(origin, action.target_cell);
            commands.spawn_scene(fireball_scene(origin, action.target_cell, *faction));
        }
        crate::timeline::end_action_until(
            &mut commands,
            entity,
            schedule.actor,
            schedule,
            executed_at,
            busy_until,
        );
    }
}

/// 到达判定：飞抵目标格中心附近就广播 [`ProjectileArrived`] 并结束飞行。
///
/// 半径判定用**真实距离**（格中心到投射物位置），因此从斜角飞来的火球
/// 也在正确的位置炸。
pub fn projectile_arrival_system(
    mut commands: Commands,
    mut arrived: MessageWriter<ProjectileArrived>,
    mut shells: Query<(Entity, &mut Transform, &mut Velocity, &Fireball, &Faction)>,
) {
    for (entity, mut transform, mut velocity, fireball, faction) in &mut shells {
        let target = fireball.target_cell.center();
        let position = transform.translation.xz();
        if position.distance(target) > ARRIVAL_TOLERANCE {
            continue;
        }
        transform.translation.x = target.x;
        transform.translation.z = target.y;
        velocity.0 = Vec3::ZERO;
        arrived.write(ProjectileArrived {
            projectile: entity,
            cell: fireball.target_cell,
            origin: Vec3::new(target.x, transform.translation.y, target.y),
            faction: *faction,
            damage: fireball.amount,
            radius: fireball.radius,
        });
        commands
            .entity(entity)
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
/// 飞行时间必须算进「行动者忙到什么时候」：火球后摇只有 0.50s，而飞行时间
/// 随距离线性增长（8 米/秒）。只按后摇恢复 [`Ready`] 的话，玩家一就绪
/// `timeline_gate_system` 就会冻结虚拟时间，火球**停在半空**——实机上看起来
/// 就像"按了技能没放出去"，但精力已经扣了。
pub fn flight_time(origin: Vec3, target_cell: Cell) -> f32 {
    let target = target_cell.center();
    let destination = Vec3::new(target.x, origin.y, target.y);
    origin.distance(destination) / FIREBALL_SPEED
}

/// 爆炸落点消息（写：到达系统；消费：爆炸结算）。
///
/// 注意：落地前的飞行时间算在行动者的忙碌窗口里（见 [`flight_time`]），
/// 所以这发球一定会在玩家重新拿到决策权之前炸掉。
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
    pub damage: f32,
    /// 爆炸半径（世界单位）
    pub radius: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_are_locked_at_declaration() {
        let fireball = Fireball {
            target_cell: Cell::new(3, -2),
            ..Fireball::default()
        };
        assert_eq!(
            fireball.target_cell,
            Cell::new(3, -2),
            "落点在声明后不应该被追着敌人改"
        );
        assert_eq!(fireball.radius, FIREBALL_RADIUS);
    }

    #[test]
    fn radius_is_expressed_in_world_units() {
        assert_eq!(
            FIREBALL_RADIUS,
            crate::timeline::CELL_SIZE * 1.5,
            "1.5 格的世界距离"
        );
    }
}
