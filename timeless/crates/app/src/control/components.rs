//! 玩家移动速度组件
use bevy::prelude::*;

/// 移动速度（格/秒）：`Velocity = axis × MoveSpeed`。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct MoveSpeed(pub f32);

impl Default for MoveSpeed {
    fn default() -> Self {
        Self(5.0)
    }
}
