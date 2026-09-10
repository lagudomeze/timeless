//! 网格化过程组件与产物标记。

use bevy::prelude::*;
use bevy::tasks::Task;

use crate::world::VoxelType;

/// 一个区块网格化的产物：按方块类型分组（同类共用一个材质）。
pub type ChunkMeshes = Vec<(VoxelType, Mesh)>;

/// 挂在区块实体上的异步网格化任务。
///
/// 任务未完成时不重复派发；区块实体被销毁会连带丢弃任务（等于取消），
/// 因此区块卸载不需要额外通知后台线程。
#[derive(Component)]
pub struct MeshingTask(pub Task<ChunkMeshes>);

/// 区块的网格实体。
///
/// 一个区块可以拆成多个网格实体（每种方块类型一个），
/// 数据与表现各自独立：销毁区块后由 [`ChunkUnloadEvent`](crate::world::ChunkUnloadEvent) 收尾。
#[derive(Component, Debug, Clone, Copy)]
pub struct ChunkSurface {
    /// 所属区块实体
    pub chunk: Entity,
    /// 本网格使用的方块类型（决定材质）
    pub voxel: VoxelType,
}
