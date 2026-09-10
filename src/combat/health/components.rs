//! 生命值组件。

use bevy::prelude::*;

/// 当前 / 最大生命值（`f32`，便于渐进扣减与百分比护盾等后续机制）。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Default for Health {
    fn default() -> Self {
        Self::new(100.0)
    }
}

impl Health {
    /// 满血单位。
    pub fn new(max: f32) -> Self {
        Self { current: max, max }
    }

    /// 是否还活着。
    pub fn is_alive(&self) -> bool {
        self.current > 0.0
    }
}
