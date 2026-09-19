//! 技能域插件：注册目录与注册消息。
//!
//! 本域**没有任何游戏系统**——它只有静态数据与一张目录。
//! 「把新交上来的定义并进目录」是维护目录自身的那一个系统，跟着数据走。

use bevy::prelude::*;

use super::registry::{RegisterAbility, SkillRegistry, apply_registrations_system};

/// 技能域插件。
#[derive(Debug, Default)]
pub struct SkillPlugin;

impl Plugin for SkillPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SkillRegistry>()
            // 注册由机制域写、本域消费（`add_message` 归消费方，见 AGENTS.md）
            .add_message::<RegisterAbility>()
            .add_systems(Update, apply_registrations_system);
    }
}
