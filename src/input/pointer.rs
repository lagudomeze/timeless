//! 鼠标输入：只翻译成消息。

use bevy::input::mouse::{AccumulatedMouseMotion, MouseButton};
use bevy::prelude::*;

use crate::presentation::PanCamera;

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
