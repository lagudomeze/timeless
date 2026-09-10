//! # voxel_render — 体素**表现**领域（渲染）
//!
//! 「画家」：把 [`crate::world`] 的体素数据变成 Mesh / 材质 / 明暗。
//! 它**只读**世界数据，从不回写规则；数据域也不认识它——两边只用
//! [`crate::world::chunk::events`] 的三个 Message 对话：
//!
//! ```text
//! ChunkDirtyEvent ─▶ schedule_meshing_system（AsyncComputeTaskPool 异步网格化）
//!                          │
//!                    MeshingTask（挂在区块实体上）
//!                          ▼
//!                    apply_meshing_result_system ─▶ 网格实体（按方块类型分材质）
//!
//! ChunkUnloadEvent ─▶ despawn_chunk_surfaces_system（清理网格实体）
//! ```
//!
//! 网格化是 CPU 密集任务，放在异步任务池里跑，避免阻塞主线程。

use bevy::prelude::*;

pub mod lighting;
pub mod materials;
pub mod meshing;
pub mod plugin;

pub use plugin::VoxelRenderPlugin;

/// 体素表现域在 `Update` 中的系统集（必须在 [`WorldSet`](crate::world::WorldSet) 之后）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoxelRenderSet;
