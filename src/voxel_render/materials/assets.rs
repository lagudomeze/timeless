//! 材质资产装配（Startup 一次）。
//!
//! 当前用纯色材质跑通链路；后续换纹理图集时，只需在这里改成
//! `TextureAtlasLayout` + UV，而不是动网格化代码。

use bevy::prelude::*;

use crate::world::VoxelType;

use super::resources::{VoxelMaterialRegistry, voxel_color};

/// 按 [`VoxelType::ALL`] 注册每种方块的 `StandardMaterial`。
pub fn setup_voxel_materials(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut registry = VoxelMaterialRegistry::default();
    for voxel in VoxelType::ALL {
        let Some(color) = voxel_color(voxel) else {
            continue;
        };
        let material = materials.add(StandardMaterial {
            base_color: color,
            // 方块表面偏粗糙，避免高光把体素棱角糊掉
            perceptual_roughness: 0.95,
            alpha_mode: if color.alpha() < 1.0 {
                AlphaMode::Blend
            } else {
                AlphaMode::Opaque
            },
            ..default()
        });
        registry.insert(voxel, material);
    }
    commands.insert_resource(registry);
    info!("🧱 已注册 {} 种方块材质", VoxelType::ALL.len() - 1);
}
