//! # 移动领域：组件 + 系统（一个文件）
//!
//! 空间位置、回合内行动组件与实时投射物（火球）：
//! - `Position`：逻辑网格坐标（展示层据此同步渲染坐标）；
//! - `Move` / `Roll`：We-Go 回合结算时的位移行动组件，决策阶段插入，
//!   由 `apply_move_intents_system` 执行位移（`Roll` 保留到裁决后供闪避系统检查）；
//! - `Projectile` + `Destination` + `ExplosionDamage`：可飞行的投射物实体，
//!   由 `projectile_system` 每帧推进，到达目的地后按爆炸伤害组件结算。

use bevy::prelude::*;

use timeless_domain::combat::HitOrder;
use timeless_domain::grid::GridPos;

use crate::combat::{BattleLog, Enemy, Health, HitLanded, Player, apply_hit};
use crate::display::map::{GRID_SIZE, cell_x, cell_x_f, cell_z, cell_z_f};
use crate::timeline::{TimeLineState, TurnPhase};

// ─────────────────────────── 组件（状态） ───────────────────────────

/// 逻辑网格位置（包装领域层 `GridPos`）
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position(pub GridPos);

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Self(GridPos::new(x, y))
    }
}

/// 移动行动：结算阶段把单位移到目标格（We-Go：每回合一次位移）
#[derive(Component, Debug, Clone, Copy)]
pub struct Move {
    pub target: Position,
}

/// 翻滚行动：结算阶段朝远离敌人方向退一格（8 向，钳制网格内），
/// 并在本回合裁决窗口内视为「闪避」（`dodge_system` 检查本组件是否存在）
#[derive(Component, Debug, Clone, Copy)]
pub struct Roll {
    /// 决策时锁定的敌人位置（远离方向以此为准，保证结算确定性）
    pub from: GridPos,
}

/// 投射物：按 `speed`（格/秒）每帧朝 `Destination` 推进
#[derive(Component, Debug, Clone, Copy)]
pub struct Projectile {
    pub speed: f32,
}

/// 投射物目的地（网格坐标）
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Destination(pub GridPos);

/// 爆炸伤害：投射物到达目的地后触发，对半径内所有 `Health` 实体结算
#[derive(Component, Debug, Clone, Copy)]
pub struct ExplosionDamage {
    pub amount: u32,
    pub radius: u32,
}

/// 火球行动：提交时锁定目标格；结算时生成投射物（敌人若本回合移动可躲避）
#[derive(Component, Debug, Clone, Copy)]
pub struct Fireball {
    pub target: Position,
    pub speed: f32,
    pub amount: u32,
    pub radius: u32,
}

/// 火球投射物共享渲染资源（setup 构建一次，施放时复用）
#[derive(Resource, Clone)]
pub struct FireballAssets {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

// ─────────────────────────── 系统 ───────────────────────────

/// 结算位移行动（Resolving 阶段，先于战斗裁决）：
/// - `Move`：直接传送到目标格，随后移除；
/// - `Roll`：远离决策时锁定的敌人一格；组件保留到裁决结束（闪避检查用），
///   由 `resolve_system` 统一清除。
///
pub fn apply_move_intents_system(
    tl: Res<TimeLineState>,
    mut commands: Commands,
    mut log: ResMut<BattleLog>,
    mut q: Query<(Entity, &mut Position, Option<&Move>, Option<&Roll>)>,
    player_q: Query<(), (With<Player>, Without<Enemy>)>,
    enemy_q: Query<(), (With<Enemy>, Without<Player>)>,
) {
    if tl.phase != TurnPhase::Resolving {
        return;
    }
    let who = |entity: Entity| -> &'static str {
        if player_q.contains(entity) {
            "玩家"
        } else if enemy_q.contains(entity) {
            "敌人"
        } else {
            "单位"
        }
    };

    for (entity, mut pos, mov, roll) in &mut q {
        if let Some(m) = mov {
            pos.0 = m.target.0;
            info!("[移动] {} → {:?}", who(entity), pos.0);
            log.push(format!(
                "[移动] {} → ({},{})",
                who(entity),
                pos.0.x,
                pos.0.y
            ));
            commands.entity(entity).remove::<Move>();
        }
        if let Some(r) = roll {
            pos.0 = retreat_from(pos.0, r.from);
            info!("[移动] {} 翻滚后退 → {:?}", who(entity), pos.0);
            log.push(format!(
                "[移动] {} 翻滚后退 → ({},{})",
                who(entity),
                pos.0.x,
                pos.0.y
            ));
            // Roll 保留：dodge_system 本回合检查；resolve_system 结束后统一清除
        }
    }
}

/// 投射物推进（每帧运行，不受回合阶段门控——火球飞行是实时表现）：
/// 沿 `Position → Destination` 方向移动 `speed * dt` 格；到达目的地后
/// 按 `ExplosionDamage` 对半径（切比雪夫距离）内所有 `Health` 实体结算，
/// 随后销毁投射物。
pub fn projectile_system(
    time: Res<Time>,
    mut commands: Commands,
    mut log: ResMut<BattleLog>,
    mut q: Query<(
        Entity,
        &mut Position,
        &mut Transform,
        &Destination,
        &Projectile,
        &ExplosionDamage,
    )>,
    mut targets: Query<(Entity, &Position, &mut Health), Without<Projectile>>,
    mut ev_hit: MessageWriter<HitLanded>,
) {
    let dt = time.delta_secs();
    for (entity, mut pos, mut tf, dest, proj, boom) in &mut q {
        let from = pos.0;
        let to = dest.0;
        if from == to {
            explode(entity, to, *boom, &mut log, &mut targets, &mut ev_hit);
            commands.entity(entity).despawn();
            continue;
        }

        let dx = (to.x - from.x) as f32;
        let dy = (to.y - from.y) as f32;
        let dist = dx.hypot(dy);
        let step = proj.speed * dt;
        if step >= dist {
            // 到达目的地：逻辑坐标落格，触发爆炸后销毁
            pos.0 = to;
            tf.translation = Vec3::new(cell_x(to.x), 0.4, cell_z(to.y));
            explode(entity, to, *boom, &mut log, &mut targets, &mut ev_hit);
            commands.entity(entity).despawn();
        } else {
            // 途中：按插值推进，渲染坐标保留小数（视觉平滑）
            let nx = from.x as f32 + dx / dist * step;
            let ny = from.y as f32 + dy / dist * step;
            pos.0 = GridPos::new(nx.round() as i32, ny.round() as i32);
            tf.translation = Vec3::new(cell_x_f(nx), 0.4, cell_z_f(ny));
        }
    }
}

/// 爆炸结算：对 `origin` 半径内的所有 `Health` 实体应用伤害并广播 `HitLanded`
fn explode(
    source: Entity,
    origin: GridPos,
    boom: ExplosionDamage,
    log: &mut BattleLog,
    targets: &mut Query<(Entity, &Position, &mut Health), Without<Projectile>>,
    ev_hit: &mut MessageWriter<HitLanded>,
) {
    info!(
        "[爆炸] 火球命中 ({},{})：{} 伤害 / 半径 {}",
        origin.x, origin.y, boom.amount, boom.radius
    );
    log.push(format!(
        "[爆炸] 火球命中 ({},{})：{} 伤害 / 半径 {}",
        origin.x, origin.y, boom.amount, boom.radius
    ));
    for (target, pos, mut hp) in targets {
        if pos.0.chebyshev(origin) <= boom.radius {
            apply_hit(
                &mut hp,
                source,
                target,
                boom.amount,
                HitOrder::Simultaneous,
                ev_hit,
            );
        }
    }
}

/// 远离敌人一格（8 向；钳制在网格范围内）
fn retreat_from(me: GridPos, foe: GridPos) -> GridPos {
    let dx = (me.x - foe.x).signum();
    let dy = (me.y - foe.y).signum();
    GridPos::new(
        (me.x + dx).clamp(0, GRID_SIZE - 1),
        (me.y + dy).clamp(0, GRID_SIZE - 1),
    )
}

/// 生成火球投射物（调试 / 技能系统共用）：
/// 实体 = `Position` + `Destination` + `Projectile` + `ExplosionDamage` + 渲染组件
#[allow(clippy::too_many_arguments)]
pub fn spawn_fireball(
    commands: &mut Commands,
    from: GridPos,
    to: GridPos,
    speed: f32,
    amount: u32,
    radius: u32,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
) {
    commands.spawn((
        Position(from),
        Destination(to),
        Projectile { speed },
        ExplosionDamage { amount, radius },
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_xyz(cell_x(from.x), 0.4, cell_z(from.y)),
        Visibility::default(),
    ));
    info!(
        "[火球] 生成：({},{}) → ({},{})，速度 {speed} 格/秒，爆炸 {amount} 伤害 / 半径 {radius}",
        from.x, from.y, to.x, to.y
    );
}
