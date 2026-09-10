//! 网格化：脏区块 → 网格数据（异步）→ 网格实体。

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
