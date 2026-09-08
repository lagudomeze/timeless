//! 攻击输入：键盘 → `FireCommand`（只翻译，不改 ECS）
use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use super::messages::{FireCommand, MeleeCommand};

/// 空格（just_pressed）→ 请求发射普通箭矢；E 键 → 请求近战横扫。
pub fn player_fire_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: MessageWriter<FireCommand>,
    mut melee_commands: MessageWriter<MeleeCommand>,
) {
    if keys.just_pressed(KeyCode::Space) {
        commands.write(FireCommand);
    }
    if keys.just_pressed(KeyCode::KeyE) {
        melee_commands.write(MeleeCommand);
    }
}
