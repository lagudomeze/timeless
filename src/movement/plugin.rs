//! 移动领域插件：注册消息与「声明 → 执行 → 位移 → 到格」系统链。

use bevy::prelude::*;

use super::MovementSet;
use super::abilities::register_abilities_system;
use super::actions::{
    declare_jump_system, declare_move_system, declare_move_to_system, jump_action_executor_system,
    jump_motion_system, move_action_executor_system,
};
use super::events::{JumpCommand, MoveCommand, MoveToCommand};
use super::systems::{follow_terrain_system, move_entities_system};

/// 移动领域插件。
#[derive(Debug, Default)]
pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        // 本域的定义是**技能目录里的条目**，所以目录必须存在：缺了就补上，
        // 否则「交上去」这一步无处可去（只装移动域的单测也不会因此炸）。
        if !app.is_plugin_added::<crate::skills::SkillPlugin>() {
            app.add_plugins(crate::skills::SkillPlugin);
        }
        app.add_message::<MoveCommand>()
            .add_message::<MoveToCommand>()
            .add_message::<JumpCommand>()
            // 可行走性被拒：写方是本域的声明系统，消费方是 presentation 的提示条
            .add_message::<super::MoveRefused>()
            // 反射：`Cell` 是决策层的坐标，BRP / 调试面板想直接读它
            .register_type::<super::Cell>()
            // 技能目录的静态数据在启动时交上去（数值仍归本域，见 `docs/skills.md`）
            .add_systems(Startup, register_abilities_system)
            .add_systems(
                Update,
                (
                    (
                        declare_move_system,
                        declare_move_to_system,
                        declare_jump_system,
                    ),
                    (move_action_executor_system, jump_action_executor_system),
                    // 先按速度位移，再贴地：贴地要在位移之后看到本帧的新位置
                    (move_entities_system, follow_terrain_system),
                    jump_motion_system,
                )
                    .chain()
                    .in_set(MovementSet),
            );
    }
}
