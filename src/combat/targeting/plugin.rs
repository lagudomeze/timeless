//! 目标获取子域插件：碰撞与近战扇形的判定系统。

use bevy::prelude::*;

use super::TargetingSet;

use super::{detect_collisions_system, detect_melee_system};

/// 打到了谁（形状相交 / 扇形）。
pub struct TargetingPlugin;

impl Plugin for TargetingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (detect_collisions_system, detect_melee_system).in_set(TargetingSet),
        );
    }
}
