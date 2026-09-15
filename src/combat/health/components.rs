//! 生命值组件。

use bevy::prelude::*;

/// 当前 / 最大生命值（整数：纯减法，可交换，没有浮点边界）。
///
/// **允许扣到负数**：伤害不提前终止，`is_alive` 只回答"还站着吗"。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Health {
    pub current: i32,
    pub max: i32,
}

impl Default for Health {
    fn default() -> Self {
        Self::new(100)
    }
}

impl Health {
    /// 满血单位。
    pub fn new(max: i32) -> Self {
        Self { current: max, max }
    }

    /// 是否还活着。
    pub fn is_alive(&self) -> bool {
        self.current > 0
    }
}
