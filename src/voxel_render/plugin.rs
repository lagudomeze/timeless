//! 体素表现域插件：材质注册 + 网格化系统链。

use bevy::prelude::*;

use super::VoxelRenderSet;
use super::materials::{VoxelMaterialRegistry, setup_voxel_materials};
use super::meshing::{
    MeshingConfig, apply_meshing_result_system, despawn_chunk_surfaces_system,
    schedule_meshing_system,
};

/// 体素表现域插件。
///
/// 启动时注册方块材质；每帧把脏区块送去异步网格化，并把结果挂成网格实体。
#[derive(Debug, Default)]
pub struct VoxelRenderPlugin;

impl Plugin for VoxelRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MeshingConfig>()
            .init_resource::<VoxelMaterialRegistry>()
            .add_systems(Startup, setup_voxel_materials)
            .add_systems(
                Update,
                (
                    schedule_meshing_system,
                    apply_meshing_result_system,
                    despawn_chunk_surfaces_system,
                )
                    .chain()
                    .in_set(VoxelRenderSet),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel_render::meshing::ChunkSurface;
    use crate::world::{ChunkLoader, WorldPlugin};
    use std::time::Duration;

    /// 端到端：区块数据 → 异步网格化 → 网格实体（不需要窗口，也不启动渲染器）。
    #[test]
    fn dirty_chunks_become_surface_entities() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .add_plugins((WorldPlugin, VoxelRenderPlugin));
        app.world_mut()
            .spawn((Transform::default(), ChunkLoader::default()));

        // 网格化在 AsyncComputeTaskPool 里跑，轮询若干帧等它完成
        let mut surfaced = 0;
        for _ in 0..100 {
            app.update();
            let mut query = app.world_mut().query::<&ChunkSurface>();
            surfaced = query.iter(app.world()).count();
            if surfaced > 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }

        assert!(surfaced > 0, "地表区块应当被网格化成网格实体");
    }
}
