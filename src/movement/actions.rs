//! 移动行动：载荷 + 工厂 + 声明 / 执行系统。
//!
//! 载荷与它的一切都住在移动领域；时间线只负责回答「到点了没有」，
//! 因此新增移动方式（冲刺、翻滚）只需在这里加载荷与执行器。
//!
//! **决策按格、表现连续**：一次移动声明走一格（[`Cell`]），执行时给一个朝格中心的
//! 速度，到位后由 [`move_entities_system`](super::systems::move_entities_system)
//! 吸附并停下。执行器自己收尾：销毁行动实体 + 把行动者忙到**真的走到位**。

use bevy::prelude::*;

use crate::timeline::{
    ActionBlocked, ActionTiming, DecisionSlot, FirstReady, Focus, FocusIntent, InputDriven,
    ScheduledAction, Uncancellable,
};

use super::cell::{Cell, MoveGoal};
use super::components::{MoveSpeed, Velocity};
use super::events::{JumpCommand, MoveCommand, MoveToCommand};

/// 移动的节奏：一格一步，几乎不设防（走得快就容易被打断）。
pub const MOVE_TIMING: ActionTiming = ActionTiming::new(0.15, 0.10, 1);

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
/// 6 与 -20 → 最高约 0.9 格、约 0.6 秒落地（正好等于 [`JUMP_TIMING`] 的后摇）。
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

/// 移动行动工厂：载荷 + 节奏 + 调度数据（移动随时可以改主意，撤销免费）。
pub fn move_action_scene(
    from_cell: Cell,
    to_cell: Cell,
    timing: ActionTiming,
    schedule: ScheduledAction,
    actor: Entity,
) -> impl Scene {
    bsn! {
        ChildOf({actor})
        MoveAction { from_cell: {from_cell}, to_cell: {to_cell} }
        template_value(timing)
        template_value(schedule)
    }
}

/// 跳跃的节奏：前摇最短；后摇覆盖整个弹道（约 0.6s），落地即可再决策。
pub const JUMP_TIMING: ActionTiming = ActionTiming::new(0.10, 0.60, 6);

/// 跳跃载荷：原地起跳、落回起跳高度。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct JumpAction;

/// 翻滚的节奏：防御性动作，几乎立即生效。
pub const ROLL_TIMING: ActionTiming = ActionTiming::new(0.05, 0.30, 1);

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

/// 翻滚行动工厂：精力在执行时才扣，因此撤销不退款也不收费。
pub fn roll_action_scene(
    from_cell: Cell,
    to_cell: Cell,
    timing: ActionTiming,
    schedule: ScheduledAction,
    actor: Entity,
) -> impl Scene {
    bsn! {
        ChildOf({actor})
        RollAction { from_cell: {from_cell}, to_cell: {to_cell} }
        template_value(timing)
        template_value(schedule)
    }
}

/// 跳跃行动工厂。
pub fn jump_action_scene(
    timing: ActionTiming,
    schedule: ScheduledAction,
    actor: Entity,
) -> impl Scene {
    bsn! {
        ChildOf({actor})
        JumpAction
        template_value(timing)
        template_value(schedule)
        // 起跳就谁都别想插队：跳跃**不给取消**（前摇里也撤不掉）
        template_value(Uncancellable)
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

/// 声明移动：`MoveCommand` → 朝该方向走一格的行动实体。
///
/// 只在玩家的决策槽是 `Empty` 时接受；输入是**按下的一次**（见 [`crate::input`] 的
/// `just_pressed` 语义），因此「按住 W」就是按一次走一格，不会和技能键互相覆盖。
pub fn declare_move_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    mut focus: ResMut<Focus>,
    intent: Res<FocusIntent>,
    mut requests: MessageReader<MoveCommand>,
    mut blocked: MessageWriter<ActionBlocked>,
    players: Query<(Entity, &Cell, &DecisionSlot), With<InputDriven>>,
) {
    let Some(axis) = requests.read().last().map(|command| command.axis) else {
        return;
    };
    let (dx, dz) = step_from_axis(axis);
    if (dx, dz) == (0, 0) {
        return;
    }
    // 忙（前摇 / 后摇 / 位移中）或没有玩家时，first_ready 会替 HUD 记下原因
    let Some((player, cell, _)) = players.iter().first_ready(&mut blocked) else {
        return;
    };

    let to_cell = Cell::new(cell.x + dx, cell.z + dz);
    let now = time.elapsed_secs();
    let schedule = ScheduledAction::with_focus(MOVE_TIMING, now, &mut focus, intent.0);
    commands.spawn_scene(move_action_scene(
        *cell,
        to_cell,
        MOVE_TIMING,
        schedule,
        player,
    ));
    commands.entity(player).insert(DecisionSlot::Windup);
}

/// 声明移动（点地板）：`MoveToCommand` → 朝目标格走**一条直线**的行动。
///
/// 多格与单格走的是**同一个载荷与执行器**（`MoveAction { from_cell, to_cell }`）：
/// 执行器本来就是"朝目标格中心设速度"，所以跨几格天然成立；忙多久也按
/// `距离 / 速度` 自动变长。
pub fn declare_move_to_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    mut focus: ResMut<Focus>,
    intent: Res<FocusIntent>,
    mut requests: MessageReader<MoveToCommand>,
    mut blocked: MessageWriter<ActionBlocked>,
    players: Query<(Entity, &Cell, &DecisionSlot), With<InputDriven>>,
) {
    let Some(target) = requests.read().last().map(|request| request.cell) else {
        return;
    };
    let Some((player, cell, _)) = players.iter().first_ready(&mut blocked) else {
        return;
    };
    if *cell == target {
        return; // 点自己脚下：不浪费一次决策
    }
    let now = time.elapsed_secs();
    let schedule = ScheduledAction::with_focus(MOVE_TIMING, now, &mut focus, intent.0);
    commands.spawn_scene(move_action_scene(
        *cell,
        target,
        MOVE_TIMING,
        schedule,
        player,
    ));
    commands.entity(player).insert(DecisionSlot::Windup);
}

/// 执行：到点的移动行动 → 朝**目标格中心**设速度，到位后由 `move_entities_system` 停下。
///
/// 收尾：行动者要忙到「人真的走到目标格」为止，而不是只忙一个后摇——
/// `Cell` 只在到位时更新，半路恢复决策槽会让下一手声明拿旧格当起点
/// （反复按 A/D 时表现为掉头 / 回弹）。
pub fn move_action_executor_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    actions: Query<(
        Entity,
        &MoveAction,
        &ActionTiming,
        &ScheduledAction,
        &ChildOf,
    )>,
    mut actors: Query<(&Cell, &MoveSpeed, &mut Velocity, &Transform)>,
) {
    let now = time.elapsed_secs();
    for (entity, action, timing, schedule, child_of) in &actions {
        if !schedule.due(now) {
            continue;
        }
        let actor = child_of.parent();
        let mut effect_delay = 0.0;
        if let Ok((_, speed, mut velocity, transform)) = actors.get_mut(actor) {
            let to_goal = action.to_cell.center() - transform.translation.xz();
            velocity.0 = ground_direction(to_goal) * speed.0;
            effect_delay = to_goal.length() / speed.0.max(f32::EPSILON);
            commands.entity(actor).insert(MoveGoal {
                cell: action.to_cell,
            });
        }
        let recovery = DecisionSlot::recovering(timing, now, effect_delay);
        commands.entity(entity).despawn();
        if let Ok(mut actor_commands) = commands.get_entity(actor) {
            actor_commands.insert(recovery);
        }
    }
}

/// 声明跳跃：`JumpCommand` → 一条跳跃行动（不可取消）。
pub fn declare_jump_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    mut focus: ResMut<Focus>,
    intent: Res<FocusIntent>,
    mut requests: MessageReader<JumpCommand>,
    mut blocked: MessageWriter<ActionBlocked>,
    players: Query<(Entity, &DecisionSlot), With<InputDriven>>,
) {
    if requests.read().last().is_none() {
        return;
    }
    let Some((player, _)) = players.iter().first_ready(&mut blocked) else {
        return; // 忙（前摇 / 后摇 / 位移中）或没有玩家
    };
    let now = time.elapsed_secs();
    let schedule = ScheduledAction::with_focus(JUMP_TIMING, now, &mut focus, intent.0);
    commands.spawn_scene(jump_action_scene(JUMP_TIMING, schedule, player));
    commands.entity(player).insert(DecisionSlot::Windup);
}

/// 执行：到点后给行动者一个向上初速度，剩下交给 [`jump_motion_system`]。
pub fn jump_action_executor_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    actions: Query<(
        Entity,
        &ActionTiming,
        &ScheduledAction,
        &JumpAction,
        &ChildOf,
    )>,
    actors: Query<&Transform>,
) {
    let now = time.elapsed_secs();
    for (entity, timing, schedule, _, child_of) in &actions {
        if !schedule.due(now) {
            continue;
        }
        let actor = child_of.parent();
        if let Ok(transform) = actors.get(actor) {
            commands.entity(actor).insert(Jumping {
                ground_y: transform.translation.y,
                velocity: JUMP_SPEED,
            });
        }
        // 后摇（0.60s）覆盖整条弹道：落地那一刻才重新可决策
        let recovery = DecisionSlot::recovering(timing, now, 0.0);
        commands.entity(entity).despawn();
        if let Ok(mut actor_commands) = commands.get_entity(actor) {
            actor_commands.insert(recovery);
        }
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
        use crate::movement::CELL_SIZE;
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

    /// 移动要忙到「人真的到位」，而不是只忙一个后摇。
    ///
    /// 一格 2.0 / 玩家速度 5.0 = 0.4s，而 MOVE 的后摇只有 0.10s。若只按后摇恢复
    /// 决策槽，玩家会在滑行途中拿到决策权，而 `Cell` 还是旧格——下一手声明
    /// 就用旧格当起点（反复按 A/D 时表现为掉头 / 回弹）。
    #[test]
    fn move_action_keeps_the_actor_busy_until_arrival() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Update, move_action_executor_system);
        let actor = app
            .world_mut()
            .spawn((
                Cell::new(0, 0),
                MoveSpeed(5.0),
                Velocity::default(),
                Transform::from_xyz(1.0, 0.0, 1.0),
            ))
            .id();
        app.world_mut().spawn((
            ChildOf(actor),
            MOVE_TIMING,
            MoveAction {
                from_cell: Cell::new(0, 0),
                to_cell: Cell::new(0, 1),
            },
            // 声明于 -1s：这条行动在"现在"已经到点了，执行器这一帧就该处理它
            ScheduledAction::declared_at(MOVE_TIMING, -1.0),
        ));

        app.update();

        let DecisionSlot::Recovery { until } = *app
            .world()
            .get::<DecisionSlot>(actor)
            .expect("执行完应当进入后摇")
        else {
            panic!("执行完应当进入后摇");
        };
        assert!(
            (until - 0.4).abs() < 1e-3,
            "忙到「走到目标格」为止的 0.4s，实际 {until}"
        );
        assert!(
            until > MOVE_TIMING.recovery,
            "必须比单纯的后摇更久，否则会半路恢复决策槽"
        );
    }

    /// 行动者阵亡 = 那条还没落地的行动跟着消失。
    ///
    /// 这条取代了旧的「行动者没了、执行器别 panic」：有了父子关系，
    /// "行动者死了但行动还在时间线上"这种状态在**结构上**就不存在了，
    /// 收尾里的 `get_entity` 守卫于是只防意外，不再是必经路径。
    #[test]
    fn an_action_dies_with_its_actor() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Update, move_action_executor_system);
        let actor = app.world_mut().spawn_empty().id();
        let action = app
            .world_mut()
            .spawn((
                ChildOf(actor),
                MOVE_TIMING,
                MoveAction::default(),
                // 声明于 -1s：如果没有跟着销毁，这一帧就会被执行
                ScheduledAction::declared_at(MOVE_TIMING, -1.0),
            ))
            .id();

        app.world_mut().entity_mut(actor).despawn();

        assert!(
            app.world().get_entity(action).is_err(),
            "行动者是行动实体的父节点：父节点销毁，行动跟着销毁"
        );

        app.update(); // 没了行动，执行器这一帧什么也不该做（更不该 panic）
    }
}
