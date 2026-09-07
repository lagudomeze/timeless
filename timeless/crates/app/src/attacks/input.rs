//! 攻击输入：键盘 → `FireCommand`（只翻译，不改 ECS）
use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use super::messages::FireCommand;

/// 空格（just_pressed）→ 请求发射普通箭矢。
pub fn player_fire_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: MessageWriter<FireCommand>,
) {
    if keys.just_pressed(KeyCode::Space) {
        commands.write(FireCommand);
    }
}
