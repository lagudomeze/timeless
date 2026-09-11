//! 移动行动：载荷 + 工厂 + 声明 / 执行系统。
//!
//! 载荷与它的一切都住在移动领域；时间线只负责「什么时候到点」，
//! 因此新增移动方式（冲刺、翻滚）只需在这里加载荷与执行器。
//!
//! **决策按格、表现连续**：一次移动声明走一格（[`Cell`]），执行时给一个朝格中心的
//! 速度，到位后由 [`move_entities_system`](super::systems::move_entities_system) 吸附并停下。

use bevy::prelude::*;

use crate::combat::Faction;
use crate::timeline::{Declared, Ready, ScheduledAction, Timeline, begin_action, timing};

use super::cell::{Cell, MoveGoal};
use super::components::{MoveSpeed, Velocity};
use super::events::{JumpCommand, MoveCommand};

/// 移动载荷：朝 `axis`（归一化平面方向）走**一格**。
///
/// 平面轴约定：`axis.x` → 世界 X，`axis.y` → 世界 **Z**（地面平面）。
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct MoveAction {
    /// 起点格（用于 HUD 画箭头 / 诊断）
    pub from_cell: Cell,
    /// 目标格（决策锁定）
    pub to_cell: Cell,
}

/// 起跳初速度（世界单位 / 秒）与重力（单位 / 秒²）。
/// 6 与 -20 → 最高约 0.9 格、约 0.6 秒落地。
const JUMP_SPEED: f32 = 6.0;
const JUMP_GRAVITY: f32 = -20.0;

/// 把平面方向吸附成「一格的正交步」。
///
/// 决策按格 ⇒ 不支持斜向半格；斜向输入取绝对值大的那个分量。
/// 分量相等时优先走 Z（W/S 方向），保证同一输入永远推出同一格。
pub fn step_from_axis(axis: Vec2) -> (i32, i32) {
    if axis == Vec2::ZERO {
        return (0, 0);
    }
    if axis.x.abs() > axis.y.abs() {
        (axis.x.signum() as i32, 0)
    } else {
        (0, axis.y.signum() as i32)
    }
}

/// 行动实体工厂：载荷 + 调度数据 + 草案标记（由 `commit_bridge_system` 升为 `Pending`）。
pub fn move_action_scene(actor: Entity, from_cell: Cell, to_cell: Cell, now: f32) -> impl Scene {
    let schedule = ScheduledAction::declared_at(actor, timing::MOVE, now);
    bsn! {
        MoveAction { from_cell: {from_cell}, to_cell: {to_cell} }
        template_value(schedule)
        Declared
    }
}

/// 跳跃载荷：原地起跳、落回起跳高度。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct JumpAction;

/// 翻滚载荷：退一格的行动实体（落地效果与无敌帧归 [`crate::combat::defense`]）。
///
/// 载荷本身住在移动领域（位移是移动的原语），只有「落地时干什么」属于防御域。
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct RollAction {
    /// 起点格（HUD 箭头 / 诊断用）
    pub from_cell: Cell,
    /// 目标格（决策锁定）
    pub to_cell: Cell,
}

/// 翻滚行动工厂。
pub fn roll_action_scene(actor: Entity, from_cell: Cell, to_cell: Cell, now: f32) -> impl Scene {
    let schedule = ScheduledAction::declared_at(actor, timing::ROLL, now);
    bsn! {
        RollAction { from_cell: {from_cell}, to_cell: {to_cell} }
        template_value(schedule)
        Declared
    }
}

/// 跳跃行动工厂。
pub fn jump_action_scene(actor: Entity, now: f32) -> impl Scene {
    let schedule = ScheduledAction::declared_at(actor, timing::JUMP, now);
    bsn! {
        JumpAction
        template_value(schedule)
        Declared
    }
}

/// 跳跃状态：窗口内的弹道；落回起跳高度后自动移除。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Jumping {
    /// 起跳高度（落地判据）
    pub ground_y: f32,
    /// 当前竖直速度
    pub velocity: f32,
}

/// 就绪的玩家查询：**必须按阵营过滤**。
///
/// 无回合模型下「就绪的单位」包含敌人，`single()` / 不加过滤的 `find()`
/// 会把玩家的动作挂到敌人身上。
type ReadyPlayer<'w, 's> =
    Query<'w, 's, (Entity, &'static Cell, &'static Faction), (With<Faction>, With<Ready>)>;

/// 声明移动：`MoveCommand` → 朝该方向走一格的行动实体。
///
/// 只在玩家有 [`Ready`] 时接受；输入是**按下的一次**（见 [`crate::input`] 的
/// `just_pressed` 语义），因此「按住 W」就是按一次走一格，不会和技能键互相覆盖。
pub fn declare_move_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    mut timeline: ResMut<Timeline>,
    mut requests: MessageReader<MoveCommand>,
    players: ReadyPlayer<'_, '_>,
) {
    let Some(axis) = requests.read().last().map(|command| command.axis) else {
        return;
    };
    let (dx, dz) = step_from_axis(axis);
    if (dx, dz) == (0, 0) {
        return;
    }
    let Ok((player, cell, _)) = players
        .iter()
        .find(|(_, _, faction)| **faction == Faction::Player)
        .ok_or(())
    else {
        return; // 忙（正在前摇 / 后摇）或没有玩家
    };

    let to_cell = Cell::new(cell.x + dx, cell.z + dz);
    let draft = commands
        .spawn_scene(move_action_scene(
            player,
            *cell,
            to_cell,
            now.elapsed_secs(),
        ))
        .id();
    begin_action(&mut commands, &mut timeline, player, draft);
}

/// 执行：到点的移动行动 → 朝**目标格中心**设速度，到位后由 `move_entities_system` 停下。
pub fn move_action_executor_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    actions: Query<(Entity, &MoveAction, &ScheduledAction), With<crate::timeline::Committed>>,
    mut actors: Query<(&Cell, &MoveSpeed, &mut Velocity, &Transform)>,
) {
    for (entity, action, schedule) in &actions {
        if let Ok((_, speed, mut velocity, transform)) = actors.get_mut(schedule.actor) {
            velocity.0 =
                ground_direction(action.to_cell.center() - transform.translation.xz()) * speed.0;
            commands.entity(schedule.actor).insert(MoveGoal {
                cell: action.to_cell,
            });
        }
        finish(
            &mut commands,
            entity,
            schedule.actor,
            schedule,
            now.elapsed_secs(),
        );
    }
}

/// 声明跳跃：`JumpCommand` → 一条跳跃行动。
pub fn declare_jump_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    mut timeline: ResMut<Timeline>,
    mut requests: MessageReader<JumpCommand>,
    players: Query<(Entity, &Faction), With<Ready>>,
) {
    if requests.read().last().is_none() {
        return;
    }
    let player = players
        .iter()
        .find(|(_, faction)| **faction == Faction::Player)
        .map(|(entity, _)| entity);
    let Some(player) = player else {
        return; // 忙（前摇 / 后摇中）或没有玩家
    };
    let draft = commands
        .spawn_scene(jump_action_scene(player, now.elapsed_secs()))
        .id();
    begin_action(&mut commands, &mut timeline, player, draft);
}

/// 执行：到点后给行动者一个向上初速度，剩下交给 [`jump_motion_system`]。
pub fn jump_action_executor_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    actions: Query<(Entity, &ScheduledAction, &JumpAction), With<crate::timeline::Committed>>,
    actors: Query<&Transform>,
) {
    for (entity, schedule, _) in &actions {
        if let Ok(transform) = actors.get(schedule.actor) {
            commands.entity(schedule.actor).insert(Jumping {
                ground_y: transform.translation.y,
                velocity: JUMP_SPEED,
            });
        }
        finish(
            &mut commands,
            entity,
            schedule.actor,
            schedule,
            now.elapsed_secs(),
        );
    }
}

/// 跳跃弹道：`v += g·dt`、`y += v·dt`，落回起跳高度就结束。
pub fn jump_motion_system(
    time: Res<Time>,
    mut commands: Commands,
    mut jumpers: Query<(Entity, &mut Transform, &mut Jumping)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut jumping) in &mut jumpers {
        jumping.velocity += JUMP_GRAVITY * dt;
        transform.translation.y += jumping.velocity * dt;
        if transform.translation.y <= jumping.ground_y {
            transform.translation.y = jumping.ground_y;
            commands.entity(entity).remove::<Jumping>();
        }
    }
}

/// 平面方向 → 世界方向：`axis.x` 走世界 X，`axis.y` 走世界 **Z**，Y 永远是 0。
///
/// 别用 `Vec2::extend`——那把第二分量放进 Y，单位会直接往天上飞。
pub fn ground_direction(axis: Vec2) -> Vec3 {
    Vec3::new(axis.x, 0.0, axis.y)
}

/// 执行器收尾：交给时间线的公共收尾（摘 `Committed` + 销毁行动实体 + 挂后摇）。
fn finish(
    commands: &mut Commands,
    action: Entity,
    actor: Entity,
    schedule: &ScheduledAction,
    executed_at: f32,
) {
    crate::timeline::end_action(commands, action, actor, schedule, executed_at);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_from_axis_snaps_to_one_orthogonal_cell() {
        assert_eq!(step_from_axis(Vec2::Y), (0, 1), "W（上）应当走 +Z 一格");
        assert_eq!(step_from_axis(Vec2::X), (1, 0));
        assert_eq!(step_from_axis(Vec2::new(0.0, -1.0)), (0, -1));
        assert_eq!(step_from_axis(Vec2::ZERO), (0, 0), "无输入不产生步伐");
    }

    #[test]
    fn diagonal_input_prefers_the_dominant_component() {
        assert_eq!(step_from_axis(Vec2::new(0.9, 0.3)), (1, 0));
        assert_eq!(step_from_axis(Vec2::new(0.3, 0.9)), (0, 1));
        // 完全相等时固定走 Z，保证同一输入永远推出同一格
        assert_eq!(step_from_axis(Vec2::ONE), (0, 1));
    }

    #[test]
    fn ground_direction_maps_the_second_component_to_z() {
        assert_eq!(ground_direction(Vec2::Y), Vec3::Z, "W（上）应当走世界 +Z");
        assert_eq!(ground_direction(Vec2::X), Vec3::X);
        assert_eq!(ground_direction(Vec2::new(0.0, -1.0)), Vec3::NEG_Z);
        assert_eq!(
            ground_direction(Vec2::ONE).y,
            0.0,
            "平面方向永远不该产生竖直速度"
        );
    }

    #[test]
    fn cell_center_round_trips_with_from_world() {
        use crate::timeline::CELL_SIZE;
        let cell = Cell::new(2, 3);
        let center = cell.center();
        assert_eq!(
            center,
            Vec2::new(2.5 * CELL_SIZE, 3.5 * CELL_SIZE),
            "格中心应当在格内"
        );
        assert_eq!(
            Cell::from_world(Vec3::new(center.x, 0.0, center.y)),
            cell,
            "格中心反推应当回到同一格"
        );
    }
}
