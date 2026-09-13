//! 地形生成：噪声参数与区块填充。

pub mod resources;
pub mod systems;

pub use resources::TerrainConfig;
pub use systems::{
    TERRAIN_CELL, generate_chunk_terrain, generate_terrain_system, ground_position, surface_height,
    surface_height_at,
};
