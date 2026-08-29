//! # 相机：主相机标记与视角控制
//!
//! 右键按住拖动平移视角（视口跟随光标），滚轮缩放（透视相机 = 改 FOV）。
//! 右键拖动在 egui 调试面板区域外生效；键盘不承担镜头移动（WASD/方向键归移动行动）。

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy_egui::EguiContexts;

use crate::display::map::GRID_SIZE;

/// 主相机标记（伪 3D 斜视角，仅一个）
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MainCamera;

/// 相机控制：右键按住拖动平移视角，滚轮缩放 FOV
pub fn camera_control_system(
    mouse: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<MouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    mut egui: EguiContexts,
    mut camera: Single<&mut Transform, With<MainCamera>>,
    mut projection: Single<&mut Projection, With<MainCamera>>,
) {
    // 右键拖动平移（抓取式：拖右/下 → 视角向左/北，地图跟随光标）
    // 灵敏度：世界单位 / 物理像素
    const DRAG_SENSITIVITY: f32 = 0.02;
    // 每帧都消费 MouseMotion 推进游标，避免按下右键时补读历史位移
    let mut delta = Vec2::ZERO;
    for m in motion.read() {
        delta += m.delta;
    }
    let over_panel = egui.ctx_mut().is_ok_and(|ctx| ctx.is_pointer_over_egui());
    if !over_panel && mouse.pressed(MouseButton::Right) && delta != Vec2::ZERO {
        let limit = GRID_SIZE as f32 / 2.0 + 3.0;
        camera.translation.x =
            (camera.translation.x - delta.x * DRAG_SENSITIVITY).clamp(-limit, limit);
        camera.translation.z =
            (camera.translation.z - delta.y * DRAG_SENSITIVITY).clamp(-limit, limit);
    }

    // 滚轮缩放
    for msg in wheel.read() {
        let Projection::Perspective(perspective) = projection.as_mut() else {
            return;
        };
        perspective.fov = (perspective.fov - msg.y * 0.03).clamp(0.3, 1.5);
    }
}
