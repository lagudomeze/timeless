//! 目标获取用组件。

use bevy::prelude::*;

/// 碰撞候选标记（临时）：目标获取系统每帧挂上，命中处理消费后移除。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollisionTarget(pub Entity);

/// 近战扇形形状：攻击实体以自身为圆心、朝 `Transform` 正前方扫过。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct MeleeShape {
    /// 触及距离
    pub range: f32,
    /// 扇形半角（弧度）
    pub half_arc: f32,
}

impl Default for MeleeShape {
    fn default() -> Self {
        Self {
            range: 2.5,
            half_arc: 60.0_f32.to_radians(),
        }
    }
}
