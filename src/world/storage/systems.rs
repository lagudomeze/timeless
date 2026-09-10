//! 体素读写：数据域对外（战斗 / 移动 / 建造）的唯一入口。

use bevy::prelude::*;

use crate::world::chunk::components::{Chunk, ChunkPos};
use crate::world::chunk::events::ChunkDirtyEvent;
use crate::world::voxel::VoxelType;

use super::resources::ChunkMap;

/// 读一个世界体素；区块未加载时返回 `None`。
pub fn get_voxel(
    chunk_map: &ChunkMap,
    chunks: &Query<&Chunk>,
    world_pos: IVec3,
) -> Option<VoxelType> {
    let pos = ChunkPos::from_voxel(world_pos);
    let entity = chunk_map.get(pos)?;
    let chunk = chunks.get(entity).ok()?;
    Some(chunk.get(pos.local(world_pos)))
}

/// 写一个世界体素。
///
/// 命中已加载区块时写入数据、发出 [`ChunkDirtyEvent`]（渲染层据此重建网格），
/// 返回 `true` 表示数据真的变了。区块未加载或数值没变都返回 `false`。
///
/// 战斗破坏地形、玩家放置方块都走这里——不直接碰区块数组，保证「改动必标脏」。
pub fn set_voxel(
    chunk_map: &ChunkMap,
    chunks: &mut Query<&mut Chunk>,
    dirty: &mut MessageWriter<ChunkDirtyEvent>,
    world_pos: IVec3,
    voxel: VoxelType,
) -> bool {
    let pos = ChunkPos::from_voxel(world_pos);
    let Some(entity) = chunk_map.get(pos) else {
        return false;
    };
    let Ok(mut chunk) = chunks.get_mut(entity) else {
        return false;
    };
    if !chunk.set(pos.local(world_pos), voxel) {
        return false;
    }
    dirty.write(ChunkDirtyEvent { chunk: entity });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::chunk::{Chunk, ChunkPos};

    /// 测试用探针：把读写 API 的结果带出系统，便于断言。
    #[derive(Resource, Default)]
    struct Probe {
        changed: bool,
        read: Option<VoxelType>,
    }

    const SAMPLE: IVec3 = IVec3::new(3, -1, 4);

    /// 写一个体素，返回是否真的变了。
    fn write_stone_system(
        chunk_map: Res<ChunkMap>,
        mut chunks: Query<&mut Chunk>,
        mut dirty: MessageWriter<ChunkDirtyEvent>,
        mut probe: ResMut<Probe>,
    ) {
        probe.changed = set_voxel(
            &chunk_map,
            &mut chunks,
            &mut dirty,
            SAMPLE,
            VoxelType::Stone,
        );
    }

    /// 读一个体素（区块索引里查得到就返回数据）。
    fn read_voxel_system(
        chunk_map: Res<ChunkMap>,
        chunks: Query<&Chunk>,
        mut probe: ResMut<Probe>,
    ) {
        probe.read = get_voxel(&chunk_map, &chunks, SAMPLE);
    }

    fn storage_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ChunkMap>()
            .init_resource::<Probe>()
            .add_message::<ChunkDirtyEvent>();
        app
    }

    #[test]
    fn set_voxel_writes_data_only_for_loaded_chunks() {
        let mut app = storage_app();
        app.add_systems(Update, write_stone_system);

        // 未加载任何区块：写入应落空
        app.update();
        assert!(!app.world().resource::<Probe>().changed);

        // 加载区块 (0, -1, 0) 后写入同一个体素
        let chunk = app
            .world_mut()
            .spawn((Chunk::empty(), ChunkPos(IVec3::new(0, -1, 0))))
            .id();
        app.world_mut()
            .resource_mut::<ChunkMap>()
            .insert(ChunkPos(IVec3::new(0, -1, 0)), chunk);
        app.update();

        assert!(
            app.world().resource::<Probe>().changed,
            "已加载区块应写入成功"
        );
        let pos = ChunkPos(IVec3::new(0, -1, 0));
        let data = app.world().get::<Chunk>(chunk).unwrap();
        assert_eq!(data.get(pos.local(IVec3::new(3, -1, 4))), VoxelType::Stone);
    }

    #[test]
    fn get_voxel_reads_through_the_chunk_index() {
        let mut app = storage_app();
        app.add_systems(Update, read_voxel_system);
        let chunk = app
            .world_mut()
            .spawn((Chunk::empty(), ChunkPos(IVec3::new(0, -1, 0))))
            .id();
        app.world_mut()
            .resource_mut::<ChunkMap>()
            .insert(ChunkPos(IVec3::new(0, -1, 0)), chunk);
        app.world_mut()
            .entity_mut(chunk)
            .get_mut::<Chunk>()
            .unwrap()
            .set(UVec3::new(3, 31, 4), VoxelType::Grass);

        app.update();
        assert_eq!(app.world().resource::<Probe>().read, Some(VoxelType::Grass));

        // 未加载的区块：读不到数据
        app.world_mut()
            .resource_mut::<ChunkMap>()
            .remove(ChunkPos(IVec3::new(0, -1, 0)));
        app.update();
        assert_eq!(
            app.world().resource::<Probe>().read,
            None,
            "未加载区块读取应返回 None"
        );
    }
}
