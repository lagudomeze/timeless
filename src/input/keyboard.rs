//! 键盘 → 消息：全部玩家输入的翻译层。

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::combat::skills::{FireCommand, MeleeCommand};
use crate::movement::MoveCommand;
use crate::timeline::ActionsCommitted;

/// WASD / 方向键 → 平面移动方向。
///
/// 每帧都写一条消息（含零方向），消费端据此设置 / 归零速度，
/// 因此不需要单独处理「松开按键」事件。
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

/// 空格 → 发射箭矢；E → 近战横扫。
pub fn player_skill_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut fire_commands: MessageWriter<FireCommand>,
    mut melee_commands: MessageWriter<MeleeCommand>,
) {
    if keys.just_pressed(KeyCode::Space) {
        fire_commands.write(FireCommand);
    }
    if keys.just_pressed(KeyCode::KeyE) {
        melee_commands.write(MeleeCommand);
    }
}

/// Enter → 提交本轮（`ActionsCommitted`）。
///
/// 提交只是「我准备好了」：时间线会把所有单位本轮的声明一起变成 `Pending`，
/// 再推进一个窗口。规划阶段之外按下不做任何事。
pub fn player_commit_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut commits: MessageWriter<ActionsCommitted>,
) {
    if keys.just_pressed(KeyCode::Enter) {
        commits.write(ActionsCommitted);
    }
}
