//! 鼠标输入：只翻译成消息。

use bevy::input::mouse::{AccumulatedMouseMotion, MouseButton, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use crate::interaction::PointerCommand;
use crate::presentation::{PanCamera, ZoomCamera};

/// 滚轮 → [`ZoomCamera`]（拉近 / 拉远；行 / 像素两种滚动单位都归一成"格"）。
pub fn camera_zoom_input_system(
    mut wheels: MessageReader<MouseWheel>,
    mut zooms: MessageWriter<ZoomCamera>,
) {
    // 累加本帧所有滚动事件，一次发一条请求
    let mut total = 0.0;
    for event in wheels.read() {
        let delta = match event.unit {
            MouseScrollUnit::Line => event.y,
            // 触摸板 / 高精度滚轮给的是像素：50px 当一格
            MouseScrollUnit::Pixel => event.y / 50.0,
        };
        total += delta;
    }
    if total != 0.0 {
        zooms.write(ZoomCamera { delta: total });
    }
}

/// 左 / 右键 → [`PointerCommand`]（只翻译，怎么解释由交互域决定）。
pub fn pointer_click_input_system(
    buttons: Res<ButtonInput<MouseButton>>,
    mut clicks: MessageWriter<PointerCommand>,
) {
    if buttons.just_pressed(MouseButton::Left) {
        clicks.write(PointerCommand::Primary);
    }
    if buttons.just_pressed(MouseButton::Right) {
        clicks.write(PointerCommand::Secondary);
    }
}

/// 按住鼠标中键拖动 → [`PanCamera`]（把本帧累计的鼠标位移交给表现域）。
pub fn camera_pan_input_system(
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut pans: MessageWriter<PanCamera>,
) {
    if !buttons.pressed(MouseButton::Middle) || motion.delta == Vec2::ZERO {
        return;
    }
    pans.write(PanCamera {
        delta: motion.delta,
    });
}
