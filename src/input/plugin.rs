//! 输入域插件。

use bevy::prelude::*;

use super::InputSet;
use super::keyboard::{
    commit_mode_toggle_system, player_commit_input_system, player_move_input_system,
    player_skill_input_system, skill_menu_input_system, skill_use_input_system,
};
use super::pointer::camera_pan_input_system;

/// 玩家输入插件：只注册「按键 → 消息」的翻译系统。
///
/// 消息本身由**消费它们的领域**注册：`MoveCommand` → movement、
/// `FireCommand` / `MeleeCommand` / 技能菜单消息 → combat、
/// `ActionsCommitted` → timeline、`PanCamera` → presentation。
/// 生产者只引用消息类型，不引用消费系统；因此单独装本插件会缺消息
/// （系统初始化即报错），要连同上面几个领域一起装。
#[derive(Debug, Default)]
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                player_move_input_system,
                player_skill_input_system,
                skill_menu_input_system,
                skill_use_input_system,
                player_commit_input_system,
                // F1 只改时间线配置（是否要 Enter 确认），不碰游戏状态
                commit_mode_toggle_system,
                camera_pan_input_system,
            )
                .in_set(InputSet),
        );
    }
}
