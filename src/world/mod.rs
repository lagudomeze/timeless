//! # world — 体素地图**数据**领域（纯数据，零渲染依赖）
//!
//! 本域只回答「世界的数据是什么」，不回答「怎么画」：区块、体素类型、地形生成与
//! 体素存取全在这里；把体素变成网格 / 材质 / 明暗的是 [`crate::voxel_render`]。
//!
//! 因此本域**不引用任何渲染类型**（`Mesh3d` / `StandardMaterial` / `Assets<Mesh>`…），
//! 只用 ECS + 数学 + 日志，可以脱离渲染环境用 `MinimalPlugins` 单测。
//!
//! ```text
//! ChunkLoader ─▶ chunk_streaming_system ─▶ ChunkLoadEvent ─▶ generate_terrain_system
//!                        │                                          │
//!                 （区块实体入 ChunkMap）                      ChunkDirtyEvent
//!                                                                   │
//!                                                       voxel_render（建网格）
//! ```
//!
//! 数据域与表现域之间只用 [`chunk::events`] 里的三个 Message 通信：
//!
//! - [`ChunkLoadEvent`]：区块实体已生成，等待填数据 / 预热；
//! - [`ChunkUnloadEvent`]：区块被卸载，表现层清理自己的产物；
//! - [`ChunkDirtyEvent`]：体素数据变了，需要重建网格。

use bevy::prelude::*;

pub mod chunk;
pub mod plugin;
pub mod storage;
pub mod terrain;
pub mod voxel;

pub use chunk::{
    CHUNK_SIZE, CHUNK_VOLUME, Chunk, ChunkDirtyEvent, ChunkLoadEvent, ChunkLoader, ChunkPinned,
    ChunkPos, ChunkUnloadEvent,
};
pub use plugin::WorldPlugin;
pub use storage::{
    BlockCommand, BlockRefused, ChunkMap, apply_block_command_system, get_voxel, set_voxel,
};
pub use terrain::{TerrainConfig, ground_position, surface_height, surface_height_at};
pub use voxel::VoxelType;

/// 世界数据域在 `Update` 中的系统集。
///
/// 领域内部顺序由 [`WorldPlugin`] 自己维护；与其他领域的先后关系只在
/// [`GamePlugin`](crate::GamePlugin) 里声明一次。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorldSet;
