//! 材质与纹理：方块类型 → 材质。

pub mod assets;
pub mod resources;

pub use assets::setup_voxel_materials;
pub use resources::{VoxelMaterialRegistry, voxel_color};
