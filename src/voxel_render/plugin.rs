//! 体素表现域插件：**只编排子域**。
//!
//! 顺序：材质先装配（网格化要拿材质句柄）→ 网格化（数据先于网格）。
//! 子域之间靠 `SystemSet` 排序，不靠插件添加顺序。

use bevy::prelude::*;

use super::VoxelRenderSet;
use super::materials::{MaterialSet, MaterialsPlugin};
use super::meshing::{MeshingPlugin, MeshingSet};

/// 体素表现插件：异步网格化 + 材质 + 明暗。
#[derive(Debug, Default)]
pub struct VoxelRenderPlugin;

impl Plugin for VoxelRenderPlugin {
    fn build(&self, app: &mut App) {
        // 明暗（`lighting`）没有插件：它是纯函数 `face_shade`，
        // 在网格化时烘进顶点色（见该子域的文首说明）。
        app.add_plugins((MaterialsPlugin, MeshingPlugin))
            .configure_sets(
                Update,
                (MaterialSet, MeshingSet).chain().in_set(VoxelRenderSet),
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
