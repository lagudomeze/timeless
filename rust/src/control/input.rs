//! 玩家输入：键盘 → `MoveCommand`（只翻译，不改 ECS）
use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use super::messages::MoveCommand;

/// WASD / 方向键 → 平面移动方向。每帧都写一条消息（包含零方向），
/// 消费端据此设置 / 归零 `Velocity`，不依赖“松开事件”单独处理。
pub fn player_move_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: MessageWriter<MoveCommand>,
) {
    let mut axis = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        axis.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        axis.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        axis.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        axis.x += 1.0;
    }
    commands.write(MoveCommand {
        axis: axis.normalize_or_zero(),
    });
}
