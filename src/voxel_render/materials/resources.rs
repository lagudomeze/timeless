//! 方块材质注册表与基色表。

use std::collections::HashMap;

use bevy::prelude::*;

use crate::world::VoxelType;

/// 方块基色（纯数据，便于单测；将来换成纹理图集索引时这里改成 UV 表）。
///
/// 返回 `None` 表示该类型不产生几何（空气）。
pub fn voxel_color(voxel: VoxelType) -> Option<Color> {
    match voxel {
        VoxelType::Air => None,
        VoxelType::Grass => Some(Color::srgb(0.30, 0.58, 0.24)),
        VoxelType::Dirt => Some(Color::srgb(0.42, 0.31, 0.20)),
        VoxelType::Stone => Some(Color::srgb(0.48, 0.48, 0.50)),
        VoxelType::Water => Some(Color::srgba(0.16, 0.38, 0.62, 0.72)),
        VoxelType::Wood => Some(Color::srgb(0.45, 0.32, 0.19)),
        VoxelType::Leaves => Some(Color::srgb(0.22, 0.45, 0.20)),
    }
}

/// 方块类型 → 材质句柄。
///
/// 一个区块按类型拆成多个网格实体，各自用这里的材质——
/// 这样同类方块共享一个材质，不同类之间也不需要多材质网格。
#[derive(Resource, Debug, Default)]
pub struct VoxelMaterialRegistry {
    materials: HashMap<VoxelType, Handle<StandardMaterial>>,
}

impl VoxelMaterialRegistry {
    /// 登记一种方块的材质。
    pub fn insert(&mut self, voxel: VoxelType, material: Handle<StandardMaterial>) {
        self.materials.insert(voxel, material);
    }

    /// 取材质；未登记的方块（如空气）返回 `None`，渲染层直接跳过。
    pub fn get(&self, voxel: VoxelType) -> Option<&Handle<StandardMaterial>> {
        self.materials.get(&voxel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_visible_voxel_type_has_a_colour() {
        for voxel in VoxelType::ALL.into_iter().filter(|v| v.is_visible()) {
            assert!(voxel_color(voxel).is_some(), "{} 缺少基色", voxel.name());
        }
        assert!(voxel_color(VoxelType::Air).is_none(), "空气不产生几何");
    }
}
