//! 材质与纹理：方块类型 → 材质。

use bevy::prelude::*;
pub mod assets;
pub mod resources;

pub use assets::setup_voxel_materials;
pub use resources::{VoxelMaterialRegistry, voxel_color};

/// 本子域系统链的位置（跨子域先后由父插件编排）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialSet;

pub mod plugin;
pub use plugin::MaterialsPlugin;
