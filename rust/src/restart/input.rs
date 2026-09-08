//! 重置输入：键盘 R → `ResetBattle`（只翻译，不改 ECS）
use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use super::systems::ResetBattle;

/// R 键（just_pressed）→ 请求重置战斗。
pub fn reset_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: MessageWriter<ResetBattle>,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        commands.write(ResetBattle);
    }
}
