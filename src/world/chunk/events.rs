//! 区块生命周期消息：数据域 → 表现域的唯一边界。
//!
//! 三条消息都遵循「谁写、谁消费」的约定，定义与生产系统同属 `world`，
//! 消费方在 `voxel_render`（新增消费方时只读消息，不改数据域）。

use bevy::prelude::*;

use super::components::ChunkPos;

/// 区块实体已生成。
///
/// 写：[`chunk_streaming_system`](super::systems::chunk_streaming_system)；
/// 消费：地形生成、表现层预热。
#[derive(Message, Debug, Clone, Copy)]
pub struct ChunkLoadEvent {
    pub chunk: Entity,
    pub pos: ChunkPos,
}

/// 区块已被卸载。
///
/// 写：[`chunk_streaming_system`](super::systems::chunk_streaming_system)；
/// 消费：表现层清理该区块的网格实体。
#[derive(Message, Debug, Clone, Copy)]
pub struct ChunkUnloadEvent {
    pub chunk: Entity,
    pub pos: ChunkPos,
}

/// 区块数据已变更，需要重建网格。
///
/// 写：地形生成 [`generate_terrain_system`](crate::world::terrain::systems::generate_terrain_system)、
/// 体素写入 [`set_voxel`](crate::world::storage::set_voxel)；
/// 消费：`voxel_render` 的网格化系统。
#[derive(Message, Debug, Clone, Copy)]
pub struct ChunkDirtyEvent {
    pub chunk: Entity,
}
