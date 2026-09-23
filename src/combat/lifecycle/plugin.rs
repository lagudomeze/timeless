//! 攻击实体生命周期子域插件：命中计数与到点清理。

use bevy::prelude::*;

use super::LifecycleSet;

use super::{cleanup_finished_attacks_system, expire_attack_entities_system};

/// 攻击实体的存活、命中计数、清理。
pub struct LifecyclePlugin;

impl Plugin for LifecyclePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                cleanup_finished_attacks_system,
                expire_attack_entities_system,
            )
                .chain()
                .in_set(LifecycleSet),
        );
    }
}
