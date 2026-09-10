//! 单个方块的实体组件。
//!
//! 区块内的体素以紧凑数组存放在 [`Chunk`](crate::world::Chunk) 里，不逐个实体化；
//! 只有需要独立交互的方块（玩家放置、可破坏方块）才 spawn 成实体挂这两个组件。

use bevy::prelude::*;

use super::types::VoxelType;

/// 一个方块实体承载的体素类型（渲染层据此选材质）。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Voxel(pub VoxelType);

/// 方块实体在世界体素坐标系中的位置（1 单位 = 1 体素）。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoxelPos(pub IVec3);
