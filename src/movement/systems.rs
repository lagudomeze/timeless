//! 移动落地系统：按速度位移，带目标格的单位走到格中心就停，并让单位贴着地形走。

use bevy::prelude::*;

use crate::world::storage::ground::ground_position_at;
use crate::world::{Chunk, ChunkMap, TerrainConfig};

use super::actions::Jumping;
use super::cell::{Cell, MoveGoal};
use super::components::Velocity;

/// 贴地爬升速度（世界单位 / 秒）。
///
/// 体素地表是台阶式的（每列差 1 个单位高），让 `y` 以这个速度追上台阶，
/// 单位看起来是"爬上去"而不是"啪一下瞬移"。到位那一帧由
/// [`move_entities_system`] 精确吸附——**不能只靠追平**：等输入时虚拟时间会冻结
/// （`dt = 0`），追到一半的单位会僵在半空。
const TERRAIN_FOLLOW_SPEED: f32 = 15.0;

/// 「到位时挂上无敌帧」的请求（翻滚用）。
///
/// 翻滚的无敌帧必须和位移**同时**生效：提前挂会在原地就无敌，
/// 推迟挂则会在飞出去之后留下破绽。因此它跟着 [`MoveGoal`] 走，
/// 由 [`move_entities_system`] 在吸附到位那一刻兑现。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct DodgingOnArrival {
    /// 无敌帧的截止时刻（虚拟秒）
    pub expires_at: f32,
}

/// 目标格的地面高度：有区块就看真实体素，没有就退回噪声。
///
/// 抽出来是因为位移与贴地两处都要这份判断，而"有没有世界数据"这件事不该
/// 在调用点各写一遍。
fn move_ground_y(
    terrain: &TerrainConfig,
    chunk_map: Option<&ChunkMap>,
    chunks: &Query<&Chunk>,
    x: f32,
    z: f32,
) -> f32 {
    match chunk_map {
        Some(chunk_map) => ground_position_at(terrain, chunk_map, chunks, x, z).y,
        None => crate::world::ground_position(terrain, x, z).y,
    }
}

/// 位移：所有带 [`Velocity`] 的实体按 `速度 × dt` 推进。
///
/// 一个系统同时处理两类实体，避免两个系统争用同一份 `Transform` / `Velocity`：
///
/// - **带 [`MoveGoal`] 的单位**：本帧位移不超过剩余距离，到格中心就吸附停下；
/// - **其余（射弹等）**：一帧走多远算多远，速度与生命周期由各自领域管理。
#[allow(clippy::type_complexity)]
pub fn move_entities_system(
    mut commands: Commands,
    time: Res<Time>,
    now: Res<Time<Virtual>>,
    terrain: Res<TerrainConfig>,
    // 站立高度要看**真实体素**（含玩家放的方块），所以这里要读区块。
    // `Option`：只装移动域的轻量单测没有 `WorldPlugin`，那时退回噪声地表
    chunk_map: Option<Res<ChunkMap>>,
    chunks: Query<&Chunk>,
    mut movers: Query<(
        Entity,
        &mut Transform,
        &mut Velocity,
        Option<&MoveGoal>,
        Option<&DodgingOnArrival>,
    )>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut velocity, goal, dodge) in &mut movers {
        if velocity.0 == Vec3::ZERO {
            continue;
        }
        let Some(goal) = goal else {
            transform.translation += velocity.0 * dt;
            continue;
        };

        let target = goal.cell.center();
        let to_target = Vec3::new(target.x, 0.0, target.y) - transform.translation.with_y(0.0);
        let remaining = to_target.length();
        let step = (velocity.0.length() * dt).min(remaining);
        if step > 0.0 {
            transform.translation += (to_target / remaining) * step;
        }

        // 到格中心：吸附（含贴地）、停下、更新决策层坐标
        if transform.translation.xz().distance(target) <= 1e-3 {
            // `y` 取目标格的地表高度：停下时一定贴着地，不受冻结时机影响
            let ground_y =
                move_ground_y(&terrain, chunk_map.as_deref(), &chunks, target.x, target.y);
            transform.translation = Vec3::new(target.x, ground_y, target.y);
            velocity.0 = Vec3::ZERO;
            let mut commands = commands.entity(entity);
            commands
                .insert(Cell::new(goal.cell.x, goal.cell.z))
                .remove::<MoveGoal>();
            if let Some(dodge) = dodge {
                commands.insert(crate::combat::defense::Dodging {
                    expires_at: dodge.expires_at.max(now.elapsed_secs()),
                });
                commands.remove::<DodgingOnArrival>();
            }
        }
    }
}

/// 贴地：站在地上的单位把 `y` 对齐到脚下的地表高度。
///
/// 为什么单独一个系统、而不是塞进位移里：**没有速度的单位也要贴地**
/// （跳跃落回、重置、将来被地形改动顶起来），而位移系统只处理"正在动"的实体。
///
/// 过滤条件就是"这是不是一个站在地上的单位"：有 [`Cell`]（投射物没有），
/// 且不在 [`Jumping`]（跳跃自己管 `y`，两边一起写会打架）。
pub fn follow_terrain_system(
    time: Res<Time>,
    terrain: Res<TerrainConfig>,
    chunk_map: Option<Res<ChunkMap>>,
    chunks: Query<&Chunk>,
    mut units: Query<&mut Transform, (With<Cell>, Without<Jumping>)>,
) {
    let max_step = TERRAIN_FOLLOW_SPEED * time.delta_secs();
    for mut transform in &mut units {
        let ground_y = match chunk_map.as_deref() {
            Some(chunk_map) => {
                ground_position_at(
                    &terrain,
                    chunk_map,
                    &chunks,
                    transform.translation.x,
                    transform.translation.z,
                )
                .y
            }
            // 没有世界数据（轻量单测）：退回噪声地表
            None => {
                crate::world::ground_position(
                    &terrain,
                    transform.translation.x,
                    transform.translation.z,
                )
                .y
            }
        };
        let delta = ground_y - transform.translation.y;
        if delta.abs() <= max_step {
            transform.translation.y = ground_y;
        } else {
            // 时间冻结时 `max_step = 0`，这里自然什么都不做
            transform.translation.y += max_step * delta.signum();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movement::ground_direction;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    /// 100ms/帧：位移与贴地都变成可预测的整数步。
    fn ground_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TerrainConfig::default())
            // 贴地要读区块（站立高度看真实体素）；这里没有区块实体，
            // 于是每一列都退回噪声地表——与改动前的行为一致
            .init_resource::<crate::world::ChunkMap>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .add_systems(
                Update,
                (move_entities_system, follow_terrain_system).chain(),
            );
        app
    }

    /// 找一对「相邻但**格中心**高度不同」的格——地形幅度是 1，一定存在。
    ///
    /// 注意比的是格中心的高度（`ground_position` 用的采样点），不是格角落的
    /// 体素列高度：一格横跨两个体素列，两者可能给出不同答案。
    fn find_step_down(config: &TerrainConfig) -> (Cell, Cell) {
        for z in 0..16 {
            for x in 0..16 {
                let from = Cell::new(x, z);
                let to = Cell::new(x + 1, z);
                if ground_y(config, from) != ground_y(config, to) {
                    return (from, to);
                }
            }
        }
        panic!("默认地形应当存在高度差");
    }

    /// 格中心的地表高度（贴地后的期望 y）。
    ///
    /// 用**噪声**函数而不是 `ground_position_at`：这些测试不造区块，
    /// 而"没有区块时站立高度 = 噪声"正是 `world::storage::ground` 保证的行为，
    /// 所以两边应当给出同一个数。
    fn ground_y(config: &TerrainConfig, cell: Cell) -> f32 {
        let center = cell.center();
        crate::world::ground_position(config, center.x, center.y).y
    }

    /// 走一格之后，单位站在**目标格**的地表高度上——而不是保持出生高度不变。
    #[test]
    fn a_moving_unit_lands_on_the_target_cells_terrain_height() {
        let mut app = ground_app();
        let terrain = *app.world().resource::<TerrainConfig>();
        let (from, to) = find_step_down(&terrain);
        let start = Vec3::new(from.center().x, ground_y(&terrain, from), from.center().y);
        let unit = app
            .world_mut()
            .spawn((
                Cell::new(from.x, from.z),
                Velocity::default(),
                MoveGoal { cell: to },
                Transform::from_translation(start),
            ))
            .id();
        let step = ground_direction(to.center() - from.center());
        app.world_mut().get_mut::<Velocity>(unit).unwrap().0 = step * 5.0;

        // 一格 2.0 / 速度 5.0 = 0.4s；跑 1 秒肯定到位
        for _ in 0..10 {
            app.update();
        }

        let transform = *app.world().get::<Transform>(unit).unwrap();
        assert_eq!(
            app.world().get::<Cell>(unit).copied(),
            Some(to),
            "到位后决策层坐标应当更新"
        );
        assert_eq!(
            transform.translation.y,
            ground_y(&terrain, to),
            "应当贴着目标格的地表，而不是停在出生高度 {}",
            start.y
        );
        assert_ne!(
            ground_y(&terrain, from),
            ground_y(&terrain, to),
            "这条测试本身要求两格高度不同"
        );
    }

    /// 时间冻结（等玩家输入）时贴地不做任何事：不会把半空的单位吸下去。
    #[test]
    fn following_does_nothing_while_time_is_frozen() {
        let mut app = ground_app();
        let terrain = *app.world().resource::<TerrainConfig>();
        let (from, _) = find_step_down(&terrain);
        let hovering = Vec3::new(from.center().x, 3.0, from.center().y);
        let unit = app
            .world_mut()
            .spawn((
                Cell::new(from.x, from.z),
                Transform::from_translation(hovering),
            ))
            .id();
        // dt = 0：等价于世界被冻结（等玩家输入时的状态）
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));

        app.update();

        assert_eq!(
            app.world().get::<Transform>(unit).unwrap().translation.y,
            3.0,
            "冻结时 dt = 0，贴地不该动"
        );
    }

    /// 投射物没有 `Cell`：贴地不许把它按到地面上。
    #[test]
    fn entities_without_a_cell_are_left_alone() {
        let mut app = ground_app();
        let projectile = app
            .world_mut()
            .spawn((
                Transform::from_xyz(1.0, 5.0, 1.0),
                Velocity(Vec3::new(0.0, 1.0, 0.0)),
            ))
            .id();

        // 通用 `Time` 在 `Last` 才从虚拟时钟拷贝，所以第一帧的 delta 还是 0：
        // 跑两帧 = 走一步（0.1s × 速度 1.0/s）
        app.update();
        app.update();

        assert_eq!(
            app.world()
                .get::<Transform>(projectile)
                .unwrap()
                .translation
                .y,
            5.1,
            "飞行中的射弹应当按自己的速度上升，而不是贴着地面"
        );
    }
}
