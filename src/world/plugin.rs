//! 世界数据域插件：**只编排子域**——注册什么、跑什么都是各子域自己的事。
//!
//! 顺序即语义：**先流式加载区块，再生成地形**（数据先于填充）。
//! 子域之间靠 `SystemSet` 排序，不靠插件添加顺序（Bevy 的 `Plugin` 添加顺序不决定系统顺序）。

use bevy::prelude::*;

use super::WorldSet;
use super::chunk::{ChunkPlugin, ChunkSet};
use super::storage::StoragePlugin;
use super::terrain::{TerrainPlugin, TerrainSet};

/// 体素地图数据域插件。
///
/// 只产出数据与事件，不接触渲染；网格化由
/// [`VoxelRenderPlugin`](crate::voxel_render::VoxelRenderPlugin) 消费事件完成。
#[derive(Debug, Default)]
pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((StoragePlugin, ChunkPlugin, TerrainPlugin))
            // 数据先于填充：区块先存在，地形才有地方写
            .configure_sets(Update, (ChunkSet, TerrainSet).chain().in_set(WorldSet));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::chunk::{Chunk, ChunkLoader, ChunkPos};
    use crate::world::storage::ChunkMap;
    use crate::world::voxel::VoxelType;

    /// 最小 App：世界数据域不需要渲染环境即可运行（分层约束的可执行证明）。
    fn world_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(WorldPlugin);
        app
    }

    #[test]
    fn streaming_loads_chunks_around_loader_and_generates_terrain() {
        let mut app = world_app();
        app.world_mut()
            .spawn((Transform::default(), ChunkLoader::default()));

        app.update();

        // 默认半径 (0, 1, 0)：加载器所在层 + 上下各一层
        let map = app.world().resource::<ChunkMap>();
        assert_eq!(map.len(), 3, "加载器周围应加载 3 个区块（y 方向三层）");
        let terrain_chunk = map
            .get(ChunkPos(IVec3::new(0, -1, 0)))
            .expect("地表层区块应被加载");
        let chunk = app.world().get::<Chunk>(terrain_chunk).unwrap();
        assert!(
            chunk.voxels.iter().any(|voxel| *voxel != VoxelType::Air),
            "区块加载后应已生成地形数据"
        );
        let air_chunk = map.get(ChunkPos(IVec3::ZERO)).unwrap();
        let chunk = app.world().get::<Chunk>(air_chunk).unwrap();
        assert!(
            chunk.voxels.iter().all(|voxel| *voxel == VoxelType::Air),
            "地表之上的区块应当是空气"
        );
    }

    #[test]
    fn streaming_unloads_chunks_left_behind_by_the_loader() {
        let mut app = world_app();
        let loader = app
            .world_mut()
            .spawn((Transform::default(), ChunkLoader::default()))
            .id();
        app.update();

        // 把加载器搬到远处的区块：旧区块卸载、新区块加载
        app.world_mut()
            .entity_mut(loader)
            .get_mut::<Transform>()
            .unwrap()
            .translation = Vec3::new(100.0, -1.0, 100.0);
        app.update();

        let map = app.world().resource::<ChunkMap>();
        assert_eq!(map.len(), 3, "离开范围的区块应被卸载");
        assert!(
            map.get(ChunkPos(IVec3::new(0, -1, 0))).is_none(),
            "旧区块应从索引中移除"
        );
        assert!(
            map.get(ChunkPos(IVec3::new(3, -1, 3))).is_some(),
            "新区块应被加载"
        );
    }
}
