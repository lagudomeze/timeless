//! 材质子域插件：按方块类型建材质的资源与 Startup 装配。

use bevy::prelude::*;

use super::{MaterialSet, VoxelMaterialRegistry, setup_voxel_materials};

/// 按类型分组的材质。
pub struct MaterialsPlugin;

impl Plugin for MaterialsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VoxelMaterialRegistry>()
            .add_systems(Startup, setup_voxel_materials.in_set(MaterialSet));
    }
}
