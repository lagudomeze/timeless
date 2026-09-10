//! AI 领域插件。

use bevy::prelude::*;

use super::AiSet;
use super::systems::{enemy_declare_system, tick_attack_cooldown_system};

/// 敌人行为插件。
#[derive(Debug, Default)]
pub struct AiPlugin;

impl Plugin for AiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (enemy_declare_system, tick_attack_cooldown_system).in_set(AiSet),
        );
    }
}
