//! 移动组件。

use bevy::prelude::*;

/// 速度（世界单位 / 秒）：位置每帧 `+= 速度 × dt`。
///
/// 玩家、敌人、投射物通用——减速、击退、飞行都只改这一个值。
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Velocity(pub Vec3);

/// 单位移动速度（格 / 秒）：`Velocity = axis × MoveSpeed`。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct MoveSpeed(pub f32);

impl Default for MoveSpeed {
    fn default() -> Self {
        Self(5.0)
    }
}
