//! 网格化子域插件：异步网格化的配置、系统链与卸载清理。

use bevy::prelude::*;

use super::{
    MeshingConfig, MeshingSet, apply_meshing_result_system, despawn_chunk_surfaces_system,
    schedule_meshing_system,
};

/// 异步面剔除网格化。
pub struct MeshingPlugin;

impl Plugin for MeshingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MeshingConfig>().add_systems(
            Update,
            (
                schedule_meshing_system,
                apply_meshing_result_system,
                despawn_chunk_surfaces_system,
            )
                .chain()
                .in_set(MeshingSet),
        );
    }
}
