//! 相机机位的**纯数学**：注视点 / 偏移 / 缩放 / 跟随目标。
//!
//! 这里没有系统、没有输入——[`CameraRig`] 只是几个数和由它们算出 `Transform` 的
//! 纯函数，所以"机位该在哪、缩放夹到多少"可以脱离 App 直接单测。
//! 消费输入（拖拽 / 滚轮）与每帧跟随在 [`super::camera`]。
//!
//! **以 PC 为画面中心**：平移改的是 [`CameraRig::pan_offset`]（**相对 PC** 的观察
//! 偏移），`focus` 由跟随系统拉向「PC + 偏移」，因此既看得见四周、又不会把自己
//! 甩出画面。

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

    /// 按「抓住地面拖动」把一段鼠标位移换算成观察偏移。
    ///
    /// 位移按相机自身的朝向投影到地面：拖右 = 镜头左移，拖下 = 镜头前移。
    /// 结果夹在 [`Self::PAN_RADIUS`] 内。
    pub fn drag_ground(&mut self, camera_rotation: Quat, delta: Vec2, per_pixel: f32) {
        let right = (camera_rotation * Vec3::X).with_y(0.0).normalize_or_zero();
        let forward = (camera_rotation * Vec3::NEG_Z)
            .with_y(0.0)
            .normalize_or_zero();
        let shift = (right * -delta.x + forward * delta.y) * per_pixel;
        self.pan_offset =
            (self.pan_offset + Vec2::new(shift.x, shift.z)).clamp_length_max(Self::PAN_RADIUS);
    }

    /// 指数趋近的插值系数：`rate` 越大越"贴"。
    ///
    /// `t = 0` 表示第一帧（直接吸附），避免开局从初始机位慢慢飘过去。
    pub fn follow_blend(&self, snapped: bool, dt: f32) -> f32 {
        if snapped {
            1.0 - (-self.follow_rate * dt).exp()
        } else {
            1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rig_places_the_camera_at_focus_plus_offset() {
        let rig = CameraRig::new(Vec3::new(4.0, 0.0, 4.0));
        let transform = rig.transform();

        assert_eq!(transform.translation, rig.focus + rig.offset);
        // 相机朝注视点看：transform 的 -Z 轴应当指向 focus
        let to_focus = (rig.focus - transform.translation).normalize();
        let forward = transform.rotation * Vec3::NEG_Z;
        assert!(
            forward.distance(to_focus) < 1e-4,
            "相机应当看向注视点：{forward:?} vs {to_focus:?}"
        );
    }

    #[test]
    fn zoom_scales_the_offset_and_stops_at_both_ends() {
        let mut rig = CameraRig::new(Vec3::ZERO);
        let base = rig.transform().translation.distance(rig.focus);

        rig.apply_zoom(100.0);
        assert_eq!(rig.zoom, CameraRig::MAX_ZOOM, "一直拉远停在最远机位");
        assert!(rig.transform().translation.distance(rig.focus) > base);

        rig.apply_zoom(-100.0);
        assert_eq!(rig.zoom, CameraRig::MIN_ZOOM, "一直拉近停在最近机位");
        assert!(rig.transform().translation.distance(rig.focus) < base);
    }

    #[test]
    fn clamp_focus_keeps_the_view_inside_the_bounds() {
        let mut rig = CameraRig::new(Vec3::ZERO);
        rig.focus = Vec3::new(999.0, 0.0, -999.0);
        rig.clamp_focus();

        assert_eq!(rig.focus.x, rig.bounds.max.x);
        assert_eq!(rig.focus.z, rig.bounds.min.y);
    }

    /// 观察偏移永远夹在半径内：拖得再远，PC 也不会被甩出画面。
    #[test]
    fn the_pan_offset_never_leaves_the_follow_radius() {
        let mut rig = CameraRig::new(Vec3::ZERO);
        let rotation = Quat::from_rotation_y(0.7);
        for _ in 0..200 {
            rig.drag_ground(rotation, Vec2::new(-400.0, -400.0), 0.05);
        }

        assert!(
            (rig.pan_offset.length() - CameraRig::PAN_RADIUS).abs() < 1e-3,
            "应当停在观察半径上，实际 {:?}",
            rig.pan_offset
        );
        assert_eq!(rig.follow_target(Vec3::ZERO).y, 0.0, "跟随目标贴在地面平面");
    }

    /// 第一帧直接吸附，之后才是指数趋近。
    #[test]
    fn the_first_frame_snaps_instead_of_easing() {
        let rig = CameraRig::new(Vec3::ZERO);
        assert_eq!(rig.follow_blend(false, 0.1), 1.0, "第一帧直接吸附");

        let eased = rig.follow_blend(true, 0.1);
        assert!(eased > 0.0 && eased < 1.0, "之后是渐近：{eased}");
        assert!(
            rig.follow_blend(true, 10.0) > eased,
            "时间越长越贴（同一帧里 dt 越大越接近目标）"
        );
    }
}
