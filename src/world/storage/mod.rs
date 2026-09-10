//! 区块索引与体素读写 API。

pub mod resources;
pub mod systems;

pub use resources::ChunkMap;
pub use systems::{get_voxel, set_voxel};
