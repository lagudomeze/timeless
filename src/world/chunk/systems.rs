//! 区块流式加载：按 [`ChunkLoader`] 覆盖范围增删区块实体。

use std::collections::HashSet;

use bevy::prelude::*;

use super::components::{Chunk, ChunkLoader, ChunkPinned, ChunkPos};
use super::events::{ChunkLoadEvent, ChunkUnloadEvent};
use crate::world::storage::ChunkMap;

/// 区块流式加载系统。
///
/// 只负责「区块实体增删 + 事件广播」，地形数据由
/// [`generate_terrain_system`](crate::world::terrain::systems::generate_terrain_system)
/// 收到 [`ChunkLoadEvent`] 后写入——两者互不感知，加载器只表达「我要多大范围」。
///
/// 没有加载器时不动作（保留现场，便于单独测试数据域）。
pub fn chunk_streaming_system(
    mut commands: Commands,
    mut chunk_map: ResMut<ChunkMap>,
    mut loads: MessageWriter<ChunkLoadEvent>,
    mut unloads: MessageWriter<ChunkUnloadEvent>,
    loaders: Query<(&Transform, &ChunkLoader)>,
    pinned: Query<(), With<ChunkPinned>>,
) {
    if loaders.is_empty() {
        return;
    }

    // 1. 汇总所有加载器覆盖的区块（切比雪夫范围，多个加载器取并集）
    let mut wanted: HashSet<ChunkPos> = HashSet::new();
    for (transform, loader) in &loaders {
        let center = ChunkPos::from_voxel(transform.translation.floor().as_ivec3());
        for dx in -loader.radius.x..=loader.radius.x {
            for dy in -loader.radius.y..=loader.radius.y {
                for dz in -loader.radius.z..=loader.radius.z {
                    wanted.insert(ChunkPos(center.0 + IVec3::new(dx, dy, dz)));
                }
            }
        }
    }

    // 2. 卸载范围外且未被钉住的区块
    let stale: Vec<(ChunkPos, Entity)> = chunk_map
        .iter()
        .filter(|(pos, _)| !wanted.contains(pos))
        .collect();
    for (pos, entity) in stale {
        if pinned.contains(entity) {
            continue;
        }
        commands.entity(entity).despawn();
        chunk_map.remove(pos);
        unloads.write(ChunkUnloadEvent { chunk: entity, pos });
    }

    // 3. 生成缺失区块（只建空数据，地形由地形系统填）
    for pos in wanted {
        if chunk_map.contains(pos) {
            continue;
        }
        let entity = commands.spawn((Chunk::empty(), pos)).id();
        chunk_map.insert(pos, entity);
        loads.write(ChunkLoadEvent { chunk: entity, pos });
    }
}
