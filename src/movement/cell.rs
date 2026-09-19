//! 格子坐标：**决策层**的坐标。
//!
//! 坐标轴约定与 A 的世界空间一致：格 `(x, z)` 对应地面平面 `(X, Z)`，
//! 高度永远取自地形 / 弹道，不进格坐标。
//!
//! 与 [`Transform`] 的分工：**决策与同格判定按格算，命中 / 射程 / 爆炸按真实距离算**
//! （见 [docs/timeline.md](../../../docs/timeline.md)）。

use bevy::prelude::*;

/// 一格的世界边长（世界单位 / 格）。
///
/// **格尺度是移动领域的真相**：它定义了 [`Cell::center`] / [`Cell::from_world`] 的换算，
/// 其他领域（战斗射程、地形量化、鼠标拾取）都只是**读**它。
/// 决策与同格判定按格算，命中 / 射程 / 爆炸按**真实距离**算，两者靠它换算。
///
/// 不取 1.0 是因为体素是 1×1×1：格子与体素一一对应会把单位压成 1 米大小，
/// 与现有模型缩放和视觉尺度对不上。
pub const CELL_SIZE: f32 = 2.0;

/// 单位当前所在格。
///
/// 只在单位**停下**时由 [`move_entities_system`](super::systems::move_entities_system)
/// 更新，不每帧从 `Transform` 反推——避免浮点抖动让格子跳变。
// `Hash` 是给「威胁覆盖了哪几格」这类集合判定用的（见 `combat::reaction`）
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[reflect(Component)]
pub struct Cell {
    pub x: i32,
    pub z: i32,
}

impl Cell {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    /// 格中心的世界坐标（地面平面，`y` 由调用方补地形高度）。
    pub fn center(self) -> Vec2 {
        Vec2::new(
            (self.x as f32 + 0.5) * CELL_SIZE,
            (self.z as f32 + 0.5) * CELL_SIZE,
        )
    }

    /// 从世界坐标取格（`floor`，因此格中心落在格内）。
    pub fn from_world(position: Vec3) -> Self {
        Self {
            x: (position.x / CELL_SIZE).floor() as i32,
            z: (position.z / CELL_SIZE).floor() as i32,
        }
    }
}

/// 当前移动目标：走到这一格就停下（见 `super::systems::move_entities_system`）。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveGoal {
    pub cell: Cell,
}
