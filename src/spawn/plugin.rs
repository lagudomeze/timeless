//! 组装车间插件：开局组装 + 运行期重新组装功能。

use bevy::prelude::*;

use super::AssemblySet;
use super::SpawnSet;
use super::assembly::setup_scene;
use super::player::equip_starting_gear_system;
use super::restart::{ResetBattle, reset_battle_system};

/// 组装车间插件。
///
/// 只注册本域的功能消息（[`ResetBattle`]，消费系统在这里，触发键在
/// [`crate::input`]）；领域消息仍由各自领域插件注册。
#[derive(Debug, Default)]
pub struct SpawnPlugin;

impl Plugin for SpawnPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ResetBattle>()
            .add_systems(Startup, setup_scene.in_set(AssemblySet))
            .add_systems(
                Update,
                // 起始装备紧跟"清场重建"之后：重置出来的玩家也要重新穿上装备。
                // 它只认「还没有槽位的玩家」，所以不是每帧都干活。
                (reset_battle_system, equip_starting_gear_system)
                    .chain()
                    .in_set(SpawnSet),
            );
    }
}
