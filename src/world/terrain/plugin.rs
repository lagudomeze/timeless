//! 地形生成子域插件：地形参数与「收到区块 → 填地形」系统。

use bevy::prelude::*;

use super::{TerrainConfig, TerrainSet, generate_terrain_system};

/// 地形生成。
pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainConfig>()
            .add_systems(Update, generate_terrain_system.in_set(TerrainSet));
    }
}
