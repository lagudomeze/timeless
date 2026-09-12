//! 表现层标记组件。

use bevy::prelude::*;

/// 主相机标记（伪 3D 斜视角，仅一个）。
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MainCamera;

/// 相机机位：看住 `focus`，相机位置固定为 `focus + offset`。
///
/// 平移只改 `focus`（贴着地面走），高度与俯角保持不变：观感稳定，也不会把镜头
/// 甩到地板底下。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct CameraRig {
    /// 注视点（世界坐标）
    pub focus: Vec3,
    /// 相机相对注视点的偏移（决定高度与俯角）
    pub offset: Vec3,
    /// 平移范围（XZ 平面，`Rect` 的 x = 世界 x、y = 世界 z）
    pub bounds: Rect,
}

impl Default for CameraRig {
    fn default() -> Self {
        Self::new(Vec3::ZERO)
    }
}

impl CameraRig {
    /// 以 `focus` 为中心的机位，平移范围默认覆盖当前区块（x/z ∈ [-8, 40]）。
    pub fn new(focus: Vec3) -> Self {
        Self {
            focus,
            offset: Vec3::new(12.0, 14.0, 12.0),
            bounds: Rect::from_corners(Vec2::new(-8.0, -8.0), Vec2::new(40.0, 40.0)),
        }
    }

    /// 由机位算出相机 Transform（位置 + 注视方向）。
    pub fn transform(&self) -> Transform {
        Transform::from_translation(self.focus + self.offset).looking_at(self.focus, Vec3::Y)
    }

    /// 把注视点限制在平移范围内。
    pub fn clamp_focus(&mut self) {
        self.focus.x = self.focus.x.clamp(self.bounds.min.x, self.bounds.max.x);
        self.focus.z = self.focus.z.clamp(self.bounds.min.y, self.bounds.max.y);
    }
}

/// HUD 状态文本标记（是否在等你决策 / 精力 / 技能 / 双方信息）。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HudStatus;

/// HUD 战斗日志文本标记。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HudLog;
