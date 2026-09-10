//! 移动落地系统：按速度位移、回合收尾停下。

use bevy::prelude::*;

use crate::timeline::RoundEnded;

use super::components::{MoveSpeed, Velocity};

/// 通用位移：所有带 [`Velocity`] 的实体按 `速度 × dt` 推进。
///
/// 速度为 0（命中结束的射弹会把自己归零）时跳过，因此本系统不认识射弹类型。
/// 规划阶段虚拟时间冻结，`dt` 为 0，位移自然停止——不需要任何 `if paused` 分支。
pub fn move_entities_system(time: Res<Time>, mut movers: Query<(&mut Transform, &Velocity)>) {
    let dt = time.delta_secs();
    for (mut transform, velocity) in &mut movers {
        if velocity.0 == Vec3::ZERO {
            continue;
        }
        transform.translation += velocity.0 * dt;
    }
}

/// 本轮结束：会走的单位停下（[`MoveSpeed`] 是单位的标志，射弹没有它）。
///
/// 飞行中的射弹不在这里清速度：它们留在时间线上，下一轮推进时接着飞。
pub fn stop_on_round_end_system(
    mut ended: MessageReader<RoundEnded>,
    mut movers: Query<&mut Velocity, With<MoveSpeed>>,
) {
    for _ in ended.read() {
        for mut velocity in &mut movers {
            velocity.0 = Vec3::ZERO;
        }
    }
}
