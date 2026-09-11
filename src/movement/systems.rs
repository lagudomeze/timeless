//! 移动落地系统：按速度位移，带目标格的单位走到格中心就停。

use bevy::prelude::*;

use super::cell::{Cell, MoveGoal};
use super::components::Velocity;

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

        // 到格中心：吸附、停下、更新决策层坐标
        if transform.translation.xz().distance(target) <= 1e-3 {
            transform.translation.x = target.x;
            transform.translation.z = target.y;
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
