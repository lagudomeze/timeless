//! 地形生成：噪声参数与区块填充。

use bevy::prelude::*;
pub mod resources;
pub mod systems;

pub use resources::TerrainConfig;
pub use systems::{
    TERRAIN_CELL, generate_chunk_terrain, generate_terrain_system, ground_position, surface_height,
    surface_height_at,
};

/// 本子域系统链的位置（跨子域先后由父插件编排）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerrainSet;

pub mod plugin;
pub use plugin::TerrainPlugin;
