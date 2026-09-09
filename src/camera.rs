use bevy::prelude::*;

/// 主相机标记（伪 3D 斜视角，仅一个）
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MainCamera;

pub fn main_camera() -> impl Scene {
    let transform = Transform::from_xyz(15.0, 15.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y);
    bsn! {
        MainCamera
        Camera3d
        template_value(transform)
    }
}

pub fn light() -> impl Scene {
    let transform = Transform::from_xyz(0.0, 18.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y);
    bsn! {
        DirectionalLight {
            color: Color::WHITE,
            illuminance: 10_000.0,
            shadow_maps_enabled: true,
        }
        template_value(transform)
    }
}
