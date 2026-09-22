//! 网格化：脏区块 → 网格数据（异步）→ 网格实体。

use bevy::prelude::*;
pub mod components;
pub mod resources;
pub mod systems;
pub mod utils;

pub use components::{ChunkMeshes, ChunkSurface, MeshingTask};
pub use resources::MeshingConfig;
pub use systems::{
    apply_meshing_result_system, despawn_chunk_surfaces_system, schedule_meshing_system,
};
pub use utils::build_chunk_meshes;

/// 本子域系统链的位置（跨子域先后由父插件编排）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshingSet;

pub mod plugin;
pub use plugin::MeshingPlugin;
