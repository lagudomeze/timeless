//! # 移动领域：网格坐标 + 位移动作 + 投射物飞行
//!
//! - `Position`：逻辑网格坐标，直接使用 Bevy 的 `IVec2`（展示层据此同步渲染坐标）；
//! - `MoveTo` / `Roll`：动作实体载荷（经时间线调度），执行时
//!   `新位置 = 当前位置 + 速度`；`Roll` 额外挂 `Dodging` 闪避标记；
//! - `Projectile` + `LinearVelocity` + `Destination`：投射物实体带位置
//!   （`Transform`）与速度，由 `projectile_motion_system` 自动飞行；
//!   飞抵目标格后广播 `ProjectileArrived`（爆炸等后果由 combat 领域消费）。
//!
//! 火球技能本体（`Fireball` / `ExplosionDamage` / `FireballAssets`）与爆炸结算
//! 属于战斗领域，见 `combat.rs`——本模块不包含任何战斗 / 技能内容。

use bevy::prelude::*;
use bevy::time::Virtual;

use crate::combat::{BattleLog, DODGE_MS, Dodging, Enemy, Player, ProjectileArrived};
use crate::display::map::{GRID_SIZE, cell_x, cell_z};
use crate::menu::{CanAttack, CanFireball, CanMove, CanRoll, MenuSelection, available_skills};
use crate::timeline::{Committed, Declared, ScheduledAction, despawn_declared_for};

// ─────────────────────────── 坐标与网格数学 ───────────────────────────

/// 逻辑网格位置（直接使用 Bevy 的 `IVec2`；应用层唯一坐标类型）
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position(pub IVec2);

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Self(IVec2::new(x, y))
    }
}

/// `IVec2` 网格数学扩展（应用层：射程判定 / 翻滚后退）。
/// 领域层裁决只接收 `u32` 距离，不依赖任何坐标类型（保持零 Bevy 依赖）。
pub(crate) trait GridMath {
    /// 切比雪夫距离（允许 8 向移动；用于射程判定：距离 1 = 相邻格）
    fn chebyshev(self, other: Self) -> u32;
    /// 远离 `foe` 一格（8 向；钳制在 `[0, grid_size-1]` 范围内）
    fn retreat_from(self, foe: Self, grid_size: i32) -> Self;
}

impl GridMath for IVec2 {
    fn chebyshev(self, other: Self) -> u32 {
        (self - other).abs().max_element() as u32
    }

    fn retreat_from(self, foe: Self, grid_size: i32) -> Self {
        (self + (self - foe).signum()).clamp(IVec2::ZERO, IVec2::splat(grid_size - 1))
    }
}

// ─────────────────────────── 位移动作载荷（动作实体） ───────────────────────────

/// 移动动作：位移速度，执行时 新位置 = 当前位置 + 速度
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct MoveTo {
    pub velocity: IVec2,
}

/// 翻滚动作：朝声明时锁定的敌人反方向退一格，并短暂进入闪避
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Roll {
    /// 决策时锁定的敌人位置（远离方向以此为准，保证结算确定性）
    pub from: IVec2,
}

// ─────────────────────────── 投射物组件 ───────────────────────────

/// 投射物标记：标识正在飞行的投射物实体（配合 `LinearVelocity` + `Destination`）
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Projectile;

/// 线性速度（世界坐标，格/秒）：带 `Transform` 的实体据此自动移动，
/// 每帧 位置 += 速度 × dt。
#[derive(Component, Debug, Clone, Copy)]
pub struct LinearVelocity(pub Vec2);

/// 投射物目的地（网格坐标）：决定到达格与落地后的爆炸位置
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Destination(pub IVec2);

// ─────────────────────────── 消息 ───────────────────────────

/// 移动方向输入（决策阶段）：dx/dy ∈ {-1, 0, 1}，由本文件的 `move_input_system` 落格。
/// 键盘（`menu::decision_keyboard_system`）与其他需要请求移动的输入源统一写本消息。
#[derive(Message, Debug, Clone, Copy)]
pub struct MoveInput {
    pub dx: i32,
    pub dy: i32,
}

// ─────────────────────────── 系统 ───────────────────────────

/// 玩家能力查询（四个 `Can*` 标记，驱动可用技能过滤）
type PlayerCapabilityQuery<'w, 's> = Query<
    'w,
    's,
    (Has<CanAttack>, Has<CanMove>, Has<CanRoll>, Has<CanFireball>),
    (With<Player>, Without<Enemy>),
>;

/// 决策输入：玩家（实体 + 当前位置）
type PlayerInputQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Position), (With<Player>, Without<Enemy>)>;

/// 移动方向落格（消费 `MoveInput`）：把方向增量叠加到玩家当前格并钳制
/// 在地图内，生成 Declared `MoveTo` 草案（替换玩家已声明的其他草案）。
/// 移动领域唯一入口——其他需要请求移动的地方都写 `MoveInput`，不重复实现。
pub fn move_input_system(
    mut menu: ResMut<MenuSelection>,
    mut commands: Commands,
    player_q: PlayerInputQuery<'_, '_>,
    capability_q: PlayerCapabilityQuery<'_, '_>,
    declared_q: Query<(Entity, &ScheduledAction), With<Declared>>,
    mut ev_move: MessageReader<MoveInput>,
) {
    let Ok((player, pos)) = player_q.single() else {
        return;
    };
    let Ok((can_attack, can_move, can_roll, can_fireball)) = capability_q.single() else {
        return;
    };
    let available = available_skills(can_attack, can_move, can_roll, can_fireball);
    if !available.contains(&1) {
        return; // 玩家无移动能力
    }

    for input in ev_move.read() {
        let target = (pos.0 + IVec2::new(input.dx, input.dy))
            .clamp(IVec2::ZERO, IVec2::splat(GRID_SIZE - 1));
        if target == pos.0 {
            continue;
        }
        despawn_declared_for(&mut commands, player, &declared_q);
        commands.spawn_scene(bsn! {
            MoveTo { velocity: {target - pos.0} }
            ScheduledAction { execute_at: 0, cast_duration: 0, actor: {player} }
            Declared
        });
        if let Some(i) = available.iter().position(|&s| s == 1) {
            menu.index = i;
        }
        info!("[决策] 移动 → ({},{})", target.x, target.y);
    }
}

/// 移动执行器：`Committed` 移动动作 → 新位置 = 当前位置 + 速度（钳制网格内）
pub fn move_executor(
    mut commands: Commands,
    q: Query<(Entity, &ScheduledAction, &MoveTo), With<Committed>>,
    mut pos_q: Query<&mut Position>,
    player_q: Query<(), With<Player>>,
    enemy_q: Query<(), With<Enemy>>,
    mut log: ResMut<BattleLog>,
) {
    let who = |entity: Entity| -> &'static str {
        if player_q.contains(entity) {
            "玩家"
        } else if enemy_q.contains(entity) {
            "敌人"
        } else {
            "单位"
        }
    };
    for (action, scheduled, mov) in &q {
        let Some(mut pos) = pos_q.get_mut(scheduled.actor).ok() else {
            commands.entity(action).despawn();
            continue;
        };
        pos.0 = (pos.0 + mov.velocity).clamp(IVec2::ZERO, IVec2::splat(GRID_SIZE - 1));
        info!(
            "[移动] {} → {:?}（速度 {:?}）",
            who(scheduled.actor),
            pos.0,
            mov.velocity
        );
        log.push(format!(
            "[移动] {} → ({},{})",
            who(scheduled.actor),
            pos.0.x,
            pos.0.y
        ));
        commands.entity(action).despawn();
    }
}

/// 翻滚执行器：`Committed` 翻滚动作 → 远离锁定敌人一格 + 挂
/// `Dodging` 闪避标记（`DODGE_MS` 后由 combat::expire_defense_markers_system 清理）
pub fn roll_executor(
    time: Res<Time<Virtual>>,
    mut commands: Commands,
    q: Query<(Entity, &ScheduledAction, &Roll), With<Committed>>,
    mut pos_q: Query<&mut Position>,
    player_q: Query<(), With<Player>>,
    enemy_q: Query<(), With<Enemy>>,
    mut log: ResMut<BattleLog>,
) {
    let who = |entity: Entity| -> &'static str {
        if player_q.contains(entity) {
            "玩家"
        } else if enemy_q.contains(entity) {
            "敌人"
        } else {
            "单位"
        }
    };
    for (action, scheduled, roll) in &q {
        let Some(mut pos) = pos_q.get_mut(scheduled.actor).ok() else {
            commands.entity(action).despawn();
            continue;
        };
        pos.0 = pos.0.retreat_from(roll.from, GRID_SIZE);
        info!("[移动] {} 翻滚后退 → {:?}", who(scheduled.actor), pos.0);
        log.push(format!(
            "[移动] {} 翻滚后退 → ({},{})",
            who(scheduled.actor),
            pos.0.x,
            pos.0.y
        ));
        commands.entity(scheduled.actor).insert(Dodging {
            expires_at: time.elapsed().as_millis() as u64 + DODGE_MS,
        });
        commands.entity(action).despawn();
    }
}

/// 投射物运动（每帧运行，受虚拟时间门控——暂停时飞行冻结）：
/// 位置（`Transform`）+ 速度（`LinearVelocity`）即自动移动；飞抵 `Destination`
/// 目标格中心后广播 `ProjectileArrived`（爆炸等后果由 combat 领域消费），
/// 并移除运动组件避免重复推进。
pub fn projectile_motion_system(
    time: Res<Time<Virtual>>,
    mut commands: Commands,
    mut q: Query<(
        Entity,
        &mut Transform,
        &LinearVelocity,
        &Destination,
        &Projectile,
    )>,
    mut ev_arrived: MessageWriter<ProjectileArrived>,
) {
    let dt = time.delta_secs();
    for (entity, mut tf, vel, dest, _marker) in &mut q {
        let step = vel.0.length() * dt;
        let target_center = Vec2::new(cell_x(dest.0.x), cell_z(dest.0.y));
        let pos = tf.translation.xz();
        if pos.distance(target_center) <= step {
            // 到达目标格：落到格中心并广播到达事件
            tf.translation.x = target_center.x;
            tf.translation.z = target_center.y;
            ev_arrived.write(ProjectileArrived {
                source: entity,
                cell: dest.0,
            });
            commands
                .entity(entity)
                .remove::<LinearVelocity>()
                .remove::<Destination>()
                .remove::<Projectile>();
        } else {
            // 途中：按速度推进（位置 += 速度 × dt）
            tf.translation += Vec3::new(vel.0.x * dt, 0.0, vel.0.y * dt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chebyshev_distance() {
        assert_eq!(IVec2::new(0, 0).chebyshev(IVec2::new(3, 4)), 4);
        assert_eq!(IVec2::new(0, 0).chebyshev(IVec2::new(1, 1)), 1);
    }

    #[test]
    fn retreat_from_clamps_inside_grid() {
        // 已在网格边缘：远离方向被钳制回界内
        assert_eq!(
            IVec2::new(0, 0).retreat_from(IVec2::new(2, 0), 5),
            IVec2::new(0, 0)
        );
        // 斜向远离
        assert_eq!(
            IVec2::new(2, 2).retreat_from(IVec2::new(3, 3), 5),
            IVec2::new(1, 1)
        );
    }
}
