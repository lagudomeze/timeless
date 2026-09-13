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
    /// 玩家拖出来的观察偏移（**相对 PC** 的地面位移，XZ 平面）。
    ///
    /// 平移改的是它、不是 `focus`：`focus` 由跟随系统拉向 `PC + pan_offset`，
    /// 这样"以 PC 为画面中心"和"能自由看四周"两件事不会互相打架。
    pub pan_offset: Vec2,
    /// 跟随收敛速度（每秒，指数趋近）。越大越"贴"，越小越"飘"。
    pub follow_rate: f32,
    /// 缩放：`offset` 按它缩放，`1.0` = 默认机位，越小越近。
    pub zoom: f32,
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
            pan_offset: Vec2::ZERO,
            follow_rate: 8.0,
            zoom: 1.0,
        }
    }

    /// 由机位算出相机 Transform（位置 + 注视方向）。
    pub fn transform(&self) -> Transform {
        Transform::from_translation(self.focus + self.offset * self.zoom)
            .looking_at(self.focus, Vec3::Y)
    }

    /// 滚轮缩放一步：`delta > 0` = 拉远（机位抬高退后），负值 = 拉近。
    pub fn apply_zoom(&mut self, delta: f32) {
        self.zoom = (self.zoom + delta).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
    }

    /// 最近机位（倍率）。
    pub const MIN_ZOOM: f32 = 0.45;
    /// 最远机位（倍率）。
    pub const MAX_ZOOM: f32 = 1.9;

    /// 把注视点限制在平移范围内。
    pub fn clamp_focus(&mut self) {
        self.focus.x = self.focus.x.clamp(self.bounds.min.x, self.bounds.max.x);
        self.focus.z = self.focus.z.clamp(self.bounds.min.y, self.bounds.max.y);
    }

    /// 跟随目标：**玩家脚下 + 观察偏移**（地面平面，`y` 恒为 0）。
    ///
    /// 把偏移限制在 [`Self::PAN_RADIUS`] 内：玩家可以四处看，但 PC 不会被甩出画面。
    pub fn follow_target(&self, player: Vec3) -> Vec3 {
        let offset = self.pan_offset.clamp_length_max(Self::PAN_RADIUS);
        Vec3::new(player.x + offset.x, 0.0, player.z + offset.y)
    }

    /// 观察偏移的最大半径（世界单位）。
    pub const PAN_RADIUS: f32 = 8.0;
}
