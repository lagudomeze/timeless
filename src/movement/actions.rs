//! 移动行动：载荷 + 工厂 + 声明 / 执行系统。
//!
//! 载荷与它的一切都住在移动领域；时间线只负责回答「到点了没有」，
//! 因此新增移动方式（冲刺、翻滚）只需在这里加载荷与执行器。
//!
//! **决策按格、表现连续**：一次移动声明走一格（[`Cell`]），执行时给一个朝格中心的
//! 速度，到位后由 [`move_entities_system`](super::systems::move_entities_system)
//! 吸附并停下。执行器自己收尾：销毁行动实体 + 把行动者忙到**真的走到位**。

use bevy::prelude::*;

use crate::config::ActionConfig;
use crate::skills::AbilityId;
use crate::timeline::{
    ActionBlocked, ActionOf, ActionTiming, DecisionSlot, FirstReady, Focus, InputDriven, Intent,
    PendingFocus, ScheduledAction, Target, Uncancellable,
};

use crate::world::{TerrainConfig, surface_height_at};

use super::cell::{CELL_SIZE, Cell, MoveGoal};
use super::components::{MoveSpeed, Velocity};
use super::events::{DashCommand, JumpCommand, MoveCommand, MoveToCommand};
use super::rules::{MoveRefused, can_step};

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

/// 从配置取节奏（没装 `ConfigPlugin` 的轻量 App 用内置常量）。
///
/// 每个声明系统都要问一遍"这一手的节奏是多少"——**只有这一处答案**，
/// 所以配置改了、目录改了、声明出来的行动三者不会分叉。
fn move_timing(config: Option<&ActionConfig>) -> ActionTiming {
    config
        .map(|config| config.move_.timing())
        .unwrap_or(MOVE_TIMING)
}

fn jump_timing(config: Option<&ActionConfig>) -> ActionTiming {
    config
        .map(|config| config.jump.timing())
        .unwrap_or(JUMP_TIMING)
}

fn dash_timing(config: Option<&ActionConfig>) -> ActionTiming {
    config
        .map(|config| config.dash.timing())
        .unwrap_or(DASH_TIMING)
}

/// 移动行动工厂：载荷 + 节奏 + 调度数据（移动随时可以改主意，撤销免费）。
pub fn move_action_scene(
    from_cell: Cell,
    to_cell: Cell,
    timing: ActionTiming,
    schedule: ScheduledAction,
    actor: Entity,
) -> impl Scene {
    // 对抗标签（能不能被打断 / 招架 / 格挡）跟着载荷一起挂在行动实体上
    let tags = super::abilities::MOVE_ABILITY.combat;
    bsn! {
        template_value(tags)
        ActionOf({actor})
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

/// 冲刺的节奏：起手比走一格重（要蹬地），后摇短（冲出去就自由了）。
///
/// **和走路的分工**：走一格的后摇短、前摇也短（0.15/0.10），但它只走一格；
/// 冲刺**一次跨两格**，前摇更重（0.25）——所以它是"用时间换距离"，
/// 追人 / 脱离时用，贴身缠斗时不如走一格灵便。
pub const DASH_TIMING: ActionTiming = ActionTiming::new(0.25, 0.20, 2);
/// 冲刺消耗的精力：比翻滚贵一点（换两格距离）。
pub const DASH_COST: u32 = 1;
/// 冲刺的位移速度（世界单位 / 秒）：比走路快，与翻滚同量级。
pub const DASH_SPEED: f32 = 7.0;
/// 冲刺一次跨几格。
pub const DASH_CELLS: i32 = 2;

/// 冲刺载荷：朝一个方向**冲两格**的行动实体。
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct DashAction {
    /// 起点格（HUD 箭头 / 诊断用）
    pub from_cell: Cell,
    /// 目标格（决策锁定）
    pub to_cell: Cell,
}

/// 冲刺行动工厂：精力在**执行时**扣（与翻滚同一条约定，见
/// [`roll_action_scene`]——所以撤销不退款，因为还没花）。
pub fn dash_action_scene(
    from_cell: Cell,
    to_cell: Cell,
    timing: ActionTiming,
    schedule: ScheduledAction,
    actor: Entity,
) -> impl Scene {
    // 对抗标签：冲刺是**起手就赌**的动作（蹬出去收不回来），与跳跃 / 翻滚同为 COMMITTED
    let tags = super::abilities::DASH_ABILITY.combat;
    bsn! {
        template_value(tags)
        ActionOf({actor})
        DashAction { from_cell: {from_cell}, to_cell: {to_cell} }
        template_value(timing)
        template_value(schedule)
    }
}

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
    // 对抗标签（能不能被打断 / 招架 / 格挡）跟着载荷一起挂在行动实体上
    let tags = super::abilities::ROLL_ABILITY.combat;
    bsn! {
        template_value(tags)
        ActionOf({actor})
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
    // 对抗标签（能不能被打断 / 招架 / 格挡）跟着载荷一起挂在行动实体上
    let tags = super::abilities::JUMP_ABILITY.combat;
    bsn! {
        template_value(tags)
        ActionOf({actor})
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
#[allow(clippy::too_many_arguments)]
pub fn declare_move_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    pending_focus: Res<PendingFocus>,
    terrain: Res<TerrainConfig>,
    config: Option<Res<ActionConfig>>,
    // 可行走性要看**真实体素**（玩家堆的墙才算墙）
    chunk_map: Option<Res<crate::world::ChunkMap>>,
    chunks: Query<&crate::world::Chunk>,
    mut requests: MessageReader<MoveCommand>,
    mut blocked: MessageWriter<ActionBlocked>,
    mut refused: MessageWriter<MoveRefused>,
    mut players: Query<(Entity, &Cell, &mut Focus, &DecisionSlot), With<InputDriven>>,
) {
    let Some(axis) = requests.read().last().map(|command| command.axis) else {
        return;
    };
    let (dx, dz) = step_from_axis(axis);
    if (dx, dz) == (0, 0) {
        return;
    }
    // 忙（前摇 / 后摇 / 位移中）或没有玩家时，first_ready 会替 HUD 记下原因
    let Some((player, cell, mut focus, _)) = players.iter_mut().first_ready(&mut blocked) else {
        return;
    };

    let to_cell = Cell::new(cell.x + dx, cell.z + dz);
    // 可行走性：高太多就是墙，走不过去（纯规则见 `super::rules`）
    if !step_is_walkable(&terrain, chunk_map.as_deref(), &chunks, *cell, to_cell) {
        refused.write(MoveRefused::BlockedByTerrain);
        return;
    }
    let timing = move_timing(config.as_deref());
    let now = time.elapsed_secs();
    let schedule = ScheduledAction::with_focus(timing, now, &mut focus, pending_focus.wants());
    commands.spawn_scene(move_action_scene(*cell, to_cell, timing, schedule, player));
    // 填意图 + 当场物化：无回合模型里没有「提交」这一步，所以声明即排期
    commands.entity(player).insert(DecisionSlot::declared(
        Intent {
            ability: AbilityId::Move,
            target: Target::Cell(to_cell),
        },
        &timing,
        now,
    ));
}

/// 声明移动（点地板）：`MoveToCommand` → 朝目标格走**一条直线**的行动。
///
/// 多格与单格走的是**同一个载荷与执行器**（`MoveAction { from_cell, to_cell }`）：
/// 执行器本来就是"朝目标格中心设速度"，所以跨几格天然成立；忙多久也按
/// `距离 / 速度` 自动变长。
#[allow(clippy::too_many_arguments)]
pub fn declare_move_to_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    pending_focus: Res<PendingFocus>,
    terrain: Res<TerrainConfig>,
    // 可行走性要看**真实体素**（玩家堆的墙才算墙）
    chunk_map: Option<Res<crate::world::ChunkMap>>,
    chunks: Query<&crate::world::Chunk>,
    mut requests: MessageReader<MoveToCommand>,
    mut blocked: MessageWriter<ActionBlocked>,
    mut refused: MessageWriter<MoveRefused>,
    mut players: Query<(Entity, &Cell, &mut Focus, &DecisionSlot), With<InputDriven>>,
) {
    let Some(target) = requests.read().last().map(|request| request.cell) else {
        return;
    };
    let Some((player, cell, mut focus, _)) = players.iter_mut().first_ready(&mut blocked) else {
        return;
    };
    if *cell == target {
        return; // 点自己脚下：不浪费一次决策
    }
    // 点地板是**一格一格走过去**的：沿途每一格的落差都得能迈上去。
    // 不查的话会出现"墙那边也点得动"——单位会直接穿墙停在墙背后。
    if !path_is_walkable(&terrain, chunk_map.as_deref(), &chunks, *cell, target) {
        refused.write(MoveRefused::BlockedByTerrain);
        return;
    }
    let now = time.elapsed_secs();
    let schedule = ScheduledAction::with_focus(MOVE_TIMING, now, &mut focus, pending_focus.wants());
    commands.spawn_scene(move_action_scene(
        *cell,
        target,
        MOVE_TIMING,
        schedule,
        player,
    ));
    // 填意图 + 当场物化（目标可能跨好几格，`MoveAction` 自己朝目标格走直线）
    commands.entity(player).insert(DecisionSlot::declared(
        Intent {
            ability: AbilityId::Move,
            target: Target::Cell(target),
        },
        &MOVE_TIMING,
        now,
    ));
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
        &ActionOf,
    )>,
    mut actors: Query<(&Cell, &MoveSpeed, &mut Velocity, &Transform)>,
) {
    let now = time.elapsed_secs();
    for (entity, action, timing, schedule, action_of) in &actions {
        if !schedule.due(now) {
            continue;
        }
        let actor = action_of.actor();
        let mut effect_delay = 0.0;
        if let Ok((_, speed, mut velocity, transform)) = actors.get_mut(actor) {
            let to_goal = action.to_cell.center() - transform.translation.xz();
            velocity.0 = ground_direction(to_goal) * speed.0;
            effect_delay = to_goal.length() / speed.0.max(f32::EPSILON);
            commands.entity(actor).insert(MoveGoal {
                cell: action.to_cell,
            });
        }
        let recovery = DecisionSlot::recovering(timing, schedule, effect_delay);
        commands.entity(entity).despawn();
        if let Ok(mut actor_commands) = commands.get_entity(actor) {
            actor_commands.insert(recovery);
        }
    }
}

/// 声明冲刺：`DashCommand` → 朝该方向冲**两格**的行动。
///
/// 与走一格（[`declare_move_system`]）共用同一套「可行走性」判据，但**沿途逐格都要过**：
/// 冲刺是"一次跨两格"，中间那一格迈不上去的话人会直接穿墙停在墙后
/// （与点地板走多格是同一个坑，见 [`path_is_walkable`]）。
///
/// 精力与翻滚一样**在执行时**扣（所以撤销不退款）。
#[allow(clippy::too_many_arguments)]
pub fn declare_dash_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    pending_focus: Res<PendingFocus>,
    terrain: Res<TerrainConfig>,
    config: Option<Res<ActionConfig>>,
    stamina_players: Query<&crate::combat::defense::Stamina>,
    chunk_map: Option<Res<crate::world::ChunkMap>>,
    chunks: Query<&crate::world::Chunk>,
    mut requests: MessageReader<DashCommand>,
    mut blocked: MessageWriter<ActionBlocked>,
    mut refused: MessageWriter<MoveRefused>,
    mut players: Query<(Entity, &Cell, &mut Focus, &DecisionSlot), With<InputDriven>>,
) {
    let Some(axis) = requests.read().last().map(|command| command.axis) else {
        return;
    };
    let (dx, dz) = step_from_axis(axis);
    if (dx, dz) == (0, 0) {
        return;
    }
    let Some((player, cell, mut focus, _)) = players.iter_mut().first_ready(&mut blocked) else {
        return;
    };
    let timing = dash_timing(config.as_deref());
    let cost = config
        .as_deref()
        .map(|config| config.dash.cost)
        .unwrap_or(DASH_COST);
    // 条件校验与其它技能同一条入口（`can_cast`）
    let def = crate::skills::AbilityDef {
        timing,
        cost: crate::skills::ResourceCost::Energy(cost),
        ..super::abilities::DASH_ABILITY
    };
    let stamina = stamina_players
        .get(player)
        .map(|stamina| stamina.current)
        .unwrap_or(0);
    if let Err(reason) = crate::skills::can_cast(&def, crate::skills::Pools::new(stamina, 0)) {
        blocked.write(ActionBlocked { reason });
        return;
    }

    let to_cell = Cell::new(cell.x + dx * DASH_CELLS, cell.z + dz * DASH_CELLS);
    // **沿途逐格**检查：冲刺跨两格，中间那格也得迈得上去
    if !path_is_walkable(&terrain, chunk_map.as_deref(), &chunks, *cell, to_cell) {
        refused.write(MoveRefused::BlockedByTerrain);
        return;
    }
    let now = time.elapsed_secs();
    let schedule = ScheduledAction::with_focus(timing, now, &mut focus, pending_focus.wants());
    commands.spawn_scene(dash_action_scene(*cell, to_cell, timing, schedule, player));
    commands.entity(player).insert(DecisionSlot::declared(
        Intent {
            ability: AbilityId::Dash,
            target: Target::Cell(to_cell),
        },
        &timing,
        now,
    ));
}

/// 执行冲刺：朝目标格设一个**冲刺速度**，到位后由 `move_entities_system` 吸附停下。
///
/// 与走一格（[`move_action_executor_system`]）唯一的区别是速度取 [`DASH_SPEED`] 而不是
/// 单位自己的 `MoveSpeed`——于是"跨两格但用时更短"这件事是**速度**说了算，
/// 不需要另一套位移逻辑（忙多久同样按 `距离 / 速度` 自动算）。
pub fn dash_action_executor_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    config: Option<Res<ActionConfig>>,
    actions: Query<(
        Entity,
        &DashAction,
        &ActionTiming,
        &ScheduledAction,
        &ActionOf,
    )>,
    mut actors: Query<(
        &Transform,
        &mut Velocity,
        &mut crate::combat::defense::Stamina,
    )>,
) {
    let now = time.elapsed_secs();
    let speed = config
        .as_deref()
        .map(|config| config.speeds.dash)
        .unwrap_or(DASH_SPEED);
    // 扣费与声明校验**读同一份配置**（声明侧见 `declare_dash_system`）
    let cost = config
        .as_deref()
        .map(|config| config.dash.cost)
        .unwrap_or(DASH_COST);
    for (entity, action, timing, schedule, action_of) in &actions {
        if !schedule.due(now) {
            continue;
        }
        let actor = action_of.actor();
        let mut effect_delay = 0.0;
        if let Ok((transform, mut velocity, mut stamina)) = actors.get_mut(actor) {
            // 执行时才扣：撤销不退款（还没花），与翻滚同一条约定
            stamina.try_spend(cost);
            let to_goal = action.to_cell.center() - transform.translation.xz();
            velocity.0 = ground_direction(to_goal) * speed;
            effect_delay = to_goal.length() / speed.max(f32::EPSILON);
            commands.entity(actor).insert(MoveGoal {
                cell: action.to_cell,
            });
        }
        let recovery = DecisionSlot::recovering(timing, schedule, effect_delay);
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
    pending_focus: Res<PendingFocus>,
    mut requests: MessageReader<JumpCommand>,
    mut blocked: MessageWriter<ActionBlocked>,
    config: Option<Res<ActionConfig>>,
    mut players: Query<(Entity, &mut Focus, &DecisionSlot), With<InputDriven>>,
) {
    if requests.read().last().is_none() {
        return;
    }
    let Some((player, mut focus, _)) = players.iter_mut().first_ready(&mut blocked) else {
        return; // 忙（前摇 / 后摇 / 位移中）或没有玩家
    };
    let timing = jump_timing(config.as_deref());
    let now = time.elapsed_secs();
    let schedule = ScheduledAction::with_focus(timing, now, &mut focus, pending_focus.wants());
    commands.spawn_scene(jump_action_scene(timing, schedule, player));
    // 原地起跳：不需要目标
    commands.entity(player).insert(DecisionSlot::declared(
        Intent {
            ability: AbilityId::Jump,
            target: Target::None,
        },
        &timing,
        now,
    ));
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
        &ActionOf,
    )>,
    actors: Query<&Transform>,
) {
    let now = time.elapsed_secs();
    for (entity, timing, schedule, _, action_of) in &actions {
        if !schedule.due(now) {
            continue;
        }
        let actor = action_of.actor();
        if let Ok(transform) = actors.get(actor) {
            commands.entity(actor).insert(Jumping {
                ground_y: transform.translation.y,
                velocity: JUMP_SPEED,
            });
        }
        // 后摇（0.60s）覆盖整条弹道：落地那一刻才重新可决策
        let recovery = DecisionSlot::recovering(timing, schedule, 0.0);
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
/// 相邻两格能不能迈过去（纯规则 + 地形高度查询）。
///
/// 高度取自 `world::surface_height_at`——**纯函数**，因此不依赖区块是否已加载，
/// 与"单位贴地"用的是同一份判据。
fn step_is_walkable(
    terrain: &TerrainConfig,
    chunk_map: Option<&crate::world::ChunkMap>,
    chunks: &Query<&crate::world::Chunk>,
    from: Cell,
    to: Cell,
) -> bool {
    let height = |cell: Cell| -> i32 {
        let center = cell.center();
        match chunk_map {
            // 看真实体素：玩家堆起来的方块因此真的挡路
            Some(chunk_map) => crate::world::storage::ground::ground_height_at(
                terrain, chunk_map, chunks, center.x, center.y,
            ),
            // 没有世界数据（轻量单测）：退回噪声地表
            None => surface_height_at(terrain, center.x, center.y),
        }
    };
    can_step(height(from), height(to))
}

/// 从 `from` 走到 `to` 的**直线路径**上，每一步都迈得过去吗。
///
/// 多格移动是**直线**（执行器朝目标格中心设速度），所以沿途经过的格就是
/// 这条直线覆盖到的格。逐格检查而不是只看终点：只看终点的话，
/// 墙可以"绕过"——单位会穿墙停在墙后面。
fn path_is_walkable(
    terrain: &TerrainConfig,
    chunk_map: Option<&crate::world::ChunkMap>,
    chunks: &Query<&crate::world::Chunk>,
    from: Cell,
    to: Cell,
) -> bool {
    let mut previous = from;
    for cell in cells_along_the_line(from, to).into_iter().skip(1) {
        if !step_is_walkable(terrain, chunk_map, chunks, previous, cell) {
            return false;
        }
        previous = cell;
    }
    true
}

/// 直线从 `from` 到 `to` 依次经过的格（含两端）。
///
/// 用细采样走一遍直线再按格去重：格是 2 个体素宽，而直线是连续的，
/// 采样步长取 `CELL_SIZE / 4` 足以不漏格（更快更粗的 Bresenham 在这里
/// 没有必要——路径最长也就几十格，而且这是**声明时**跑一次，不是每帧）。
fn cells_along_the_line(from: Cell, to: Cell) -> Vec<Cell> {
    let start = from.center();
    let end = to.center();
    let delta = end - start;
    let length = delta.length();
    if length <= f32::EPSILON {
        return vec![from];
    }
    let steps = (length / (CELL_SIZE / 4.0)).ceil().max(1.0) as usize;
    let mut cells = vec![from];
    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        let point = start + delta * t;
        let cell = Cell::new(point.x.floor() as i32, point.y.floor() as i32);
        if cells.last() != Some(&cell) {
            cells.push(cell);
        }
    }
    cells
}

pub fn ground_direction(axis: Vec2) -> Vec3 {
    Vec3::new(axis.x, 0.0, axis.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    /// **可行走性真的接在声明上**：地形高得迈不上去时，声明被拒、不产生行动。
    ///
    /// 默认地形相邻格最多差 1 个体素（处处可走），所以这里把起伏调大，
    /// 造出一堵真的走不过去的"墙"——正是玩家自己堆高地形时的情形。
    #[test]
    fn a_target_behind_a_wall_is_refused() {
        use crate::timeline::PendingFocus;
        use crate::timeline::{ActionBlocked, DecisionSlot, Focus, InputDriven};

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            // 幅度拉大：制造相邻 2 级以上的落差
            .insert_resource(TerrainConfig {
                amplitude: 8,
                base_height: 0,
                scale: 3.0,
                ..TerrainConfig::default()
            })
            .init_resource::<PendingFocus>()
            .add_message::<MoveCommand>()
            .add_message::<ActionBlocked>()
            .add_message::<MoveRefused>()
            .add_systems(Update, declare_move_system);

        // 找一对"迈不上去"的相邻格
        let terrain = *app.world().resource::<TerrainConfig>();
        let mut found = None;
        'outer: for z in -8..8 {
            for x in -8..8 {
                let from = Cell::new(x, z);
                let to = Cell::new(x + 1, z);
                let walkable =
                    with_empty_chunks(|chunks| step_is_walkable(&terrain, None, chunks, from, to));
                if !walkable {
                    found = Some((from, to));
                    break 'outer;
                }
            }
        }
        let (from, to) = found.expect("放大起伏后应当存在迈不上去的相邻格");

        let player = app
            .world_mut()
            .spawn((
                InputDriven,
                from,
                Focus::default(),
                DecisionSlot::Idle { intent: None },
            ))
            .id();
        app.world_mut().write_message(MoveCommand {
            axis: Vec2::new(1.0, 0.0),
        });
        // 只声明、不执行：这里要看的正是"声明有没有被拒"
        app.update();

        assert_eq!(
            slot_of(&mut app, player),
            DecisionSlot::Idle { intent: None },
            "走不过去就不该占用决策（槽必须还是空的）"
        );
        assert_eq!(actions_of(&mut app), 0, "被拒的移动不该产生行动实体");
        let _ = to;
    }

    /// 同一条直线上的**每一格**都要能迈上去：墙不能被"绕过"。
    #[test]
    fn every_cell_on_the_line_must_be_walkable() {
        use crate::timeline::PendingFocus;
        let _ = PendingFocus; // 仅供可读性：这个断言不碰调度

        let terrain = TerrainConfig {
            amplitude: 8,
            base_height: 0,
            scale: 3.0,
            ..TerrainConfig::default()
        };
        // 找一条"起点与终点之间隔着墙"的直线
        let mut demonstrated = false;
        for z in -12..12 {
            for x in -12..12 {
                let from = Cell::new(x, z);
                let to = Cell::new(x + 6, z);
                let endpoints_ok = with_empty_chunks(|chunks| {
                    step_is_walkable(&terrain, None, chunks, from, Cell::new(x + 1, z))
                        && step_is_walkable(&terrain, None, chunks, Cell::new(x + 5, z), to)
                });
                let blocked =
                    with_empty_chunks(|chunks| path_is_walkable(&terrain, None, chunks, from, to));
                if endpoints_ok && !blocked {
                    demonstrated = true;
                    break;
                }
            }
            if demonstrated {
                break;
            }
        }
        assert!(
            demonstrated,
            "应当存在「两端可走、中间被墙挡住」的直线——这正是逐格检查的意义"
        );
    }

    /// 一个**真实的空查询**：单测没有世界数据，因此走"退回噪声"那条分支。
    ///
    /// `Query` 是系统参数类型，脱离 App 构造不出来——所以借 `SystemState`
    /// 从一个小 App 里取一个。这不是权宜之计：它证明"没有区块时行为不变"，
    /// 而那条正是 [`crate::world::storage::ground`] 承诺的兜底。
    fn with_empty_chunks<R>(f: impl FnOnce(&Query<&crate::world::Chunk>) -> R) -> R {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let mut state =
            bevy::ecs::system::SystemState::<Query<&crate::world::Chunk>>::new(app.world_mut());
        let query = state.get(app.world()).unwrap();
        f(&query)
    }

    /// 读行动者的决策槽。
    fn slot_of(app: &mut App, entity: Entity) -> DecisionSlot {
        *app.world().get::<DecisionSlot>(entity).unwrap()
    }

    /// 场上还剩几条行动实体。
    fn actions_of(app: &mut App) -> usize {
        let mut query = app.world_mut().query::<&ActionOf>();
        query.iter(app.world()).count()
    }

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
            // 手动时钟：执行器要一个会走的 `now` 才能判「到点了吗」
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
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
            ActionOf(actor),
            MOVE_TIMING,
            MoveAction {
                from_cell: Cell::new(0, 0),
                to_cell: Cell::new(0, 1),
            },
            // 前摇刚好走完：`due` 是严格大于，所以取 0.0 而让它在本帧（0.1）到点。
            // 后摇从 `execute_at` 起算，因此忙碌窗口应当是 0.0 + 0.4。
            ScheduledAction::immediate(0.0),
        ));

        app.update();
        // 执行器用 `Commands` 写槽，落地下一个 apply 才生效
        app.update();

        let DecisionSlot::Executing { until } = *app
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
    /// 这条取代了旧的「行动者没了、执行器别 panic」：归属关系标了 `linked_spawn`，
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
                ActionOf(actor),
                MOVE_TIMING,
                MoveAction::default(),
                // 声明于 -1s：如果没有跟着销毁，这一帧就会被执行
                ScheduledAction::declared_at(MOVE_TIMING, -1.0),
            ))
            .id();

        app.world_mut().entity_mut(actor).despawn();

        assert!(
            app.world().get_entity(action).is_err(),
            "行动者是这条行动的归属方：行动者销毁，行动跟着销毁（linked_spawn）"
        );

        app.update(); // 没了行动，执行器这一帧什么也不该做（更不该 panic）
    }
}
