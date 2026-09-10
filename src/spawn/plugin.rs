//! 组装车间插件：开局组装 + 运行期重新组装功能。

use bevy::prelude::*;

use super::AssemblySet;
use super::SpawnSet;
use super::assembly::setup_scene;
use super::restart::{ResetBattle, reset_battle_system, restart_input_system};

/// 组装车间插件。
///
/// 只注册本域的功能消息（[`ResetBattle`]，功能自带触发键与消费系统）；
/// 领域消息仍由各自领域插件注册。
#[derive(Debug, Default)]
pub struct SpawnPlugin;

impl Plugin for SpawnPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ResetBattle>()
            .add_systems(Startup, setup_scene.in_set(AssemblySet))
            .add_systems(
                Update,
                (restart_input_system, reset_battle_system)
                    .chain()
                    .in_set(SpawnSet),
            );
    }
}
