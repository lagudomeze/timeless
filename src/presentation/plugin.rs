//! 表现领域插件：资源预载 + 相机平移 + 战斗日志 + HUD。

use bevy::prelude::*;

use super::PreloadSet;
use super::PresentationSet;
use super::camera::{PanCamera, camera_pan_system};
use super::hud::{setup_hud, update_hud_system};
use super::log::{BattleLog, battle_log_system};
use super::preload::preload;

/// 表现领域插件。
#[derive(Debug, Default)]
pub struct PresentationPlugin;

impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BattleLog>()
            .add_message::<PanCamera>()
            .add_systems(Startup, (preload.in_set(PreloadSet), setup_hud))
            .add_systems(
                Update,
                (camera_pan_system, battle_log_system, update_hud_system)
                    .chain()
                    .in_set(PresentationSet),
            );
    }
}
