//! 材质资产装配（Startup 一次）。
//!
//! 当前用纯色材质跑通链路（顶点色承载面朝向明暗，见
//! [`crate::voxel_render::lighting`]）。
//!
//! ⚠️ **换纹理图集不是"只改这里"**：网格现在**没有 UV**（`MeshBuilder` 只写
//! 位置 / 法线 / 顶点色），而贪婪网格化之后 UV 还要**按矩形尺寸铺开**
//! （合并出来的面跨 N 个格子，贴图得重复 N 次）——所以网格化那边也要改。
//! 分工保持不变：**图集怎么切分住本文件，网格只知道"这一面要铺几格"**。

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
