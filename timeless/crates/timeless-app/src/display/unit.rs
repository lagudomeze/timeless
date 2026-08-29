//! # 战斗单位表现：纸片（Billboard）+ 贴地阴影（GroundShadow）
//!
//! 单位根节点承载逻辑坐标（`Position`），纸片与阴影作为子实体跟随；
//! 根节点不旋转（阴影保持水平），纸片子实体每帧朝向主相机。

use bevy::prelude::*;

use crate::display::camera::MainCamera;
use crate::display::map::{cell_x, cell_z};
use crate::movement::Position;

/// 纸片高度 / 宽度与阴影半径（PaperAssets 构建与子实体摆放共用）
pub(crate) const BILLBOARD_HEIGHT: f32 = 1.2;
pub(crate) const BILLBOARD_WIDTH: f32 = 0.8;
pub(crate) const SHADOW_RADIUS: f32 = 0.36;

/// 战斗单位根节点：承载逻辑坐标（`Position`）与领域组件；
/// 纸片与贴地阴影作为子实体跟随，父节点不旋转（阴影保持水平）。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitRoot;

/// 2D 纸片单位：每帧朝向主相机（billboard）
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Billboard;

/// 贴地阴影：单位根节点的子实体，随根节点移动；本组件仅作标记
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroundShadow;

/// 纸片单位与阴影的共享渲染资源（setup 构建一次，重置/生成时复用）
#[derive(Resource, Clone)]
pub struct PaperAssets {
    pub billboard: Handle<Mesh>,
    pub shadow: Handle<Mesh>,
    pub player: Handle<StandardMaterial>,
    pub enemy: Handle<StandardMaterial>,
    pub shadow_mat: Handle<StandardMaterial>,
}

/// 逻辑坐标 → 渲染坐标（单位根节点；纸片与阴影子实体随根节点自动跟随）
pub fn sync_transforms(mut q: Query<(&Position, &mut Transform), With<UnitRoot>>) {
    for (pos, mut tf) in &mut q {
        tf.translation = Vec3::new(cell_x(pos.0.x), 0.0, cell_z(pos.0.y));
    }
}

/// 纸片单位朝向主相机（billboard）
pub fn billboard_system(
    cam: Single<&GlobalTransform, With<MainCamera>>,
    mut q: Query<&mut Transform, With<Billboard>>,
) {
    let cam_pos = cam.translation();
    for mut tf in &mut q {
        tf.look_at(cam_pos, Vec3::Y);
    }
}
