//! 网格化系统：脏区块 → 异步任务 → 网格实体。

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, block_on, futures_lite::future};

use crate::voxel_render::materials::VoxelMaterialRegistry;
use crate::world::chunk::events::{ChunkDirtyEvent, ChunkUnloadEvent};
use crate::world::{Chunk, ChunkPos};

use super::components::{ChunkSurface, MeshingTask};
use super::resources::MeshingConfig;
use super::utils::build_chunk_meshes;

/// 脏区块 → 后台网格化任务。
///
/// CPU 密集的面剔除放在 `AsyncComputeTaskPool`，主线程只负责派发；
/// 同一帧重复标脏只派发一次，新的数据会覆盖旧任务（旧任务被丢弃即取消）。
pub fn schedule_meshing_system(
    mut commands: Commands,
    mut dirty: MessageReader<ChunkDirtyEvent>,
    config: Res<MeshingConfig>,
    chunks: Query<&Chunk>,
) {
    let mut scheduled: Vec<Entity> = Vec::new();
    for event in dirty.read() {
        if scheduled.contains(&event.chunk) {
            continue;
        }
        let Ok(chunk) = chunks.get(event.chunk) else {
            continue; // 区块在这一帧被卸载：网格也随之作废
        };
        let data = chunk.clone();
        let config = *config;
        let task =
            AsyncComputeTaskPool::get().spawn(async move { build_chunk_meshes(&data, config) });
        commands.entity(event.chunk).insert(MeshingTask(task));
        scheduled.push(event.chunk);
    }
}

/// 任务完成 → 用新网格替换该区块的表现实体。
///
/// 一次重建会先清掉该区块的旧网格实体，再按方块类型各挂一个网格实体：
/// 「一个区块 = 多个网格实体」让每种方块用单一材质，避免多材质网格的复杂度。
pub fn apply_meshing_result_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    registry: Res<VoxelMaterialRegistry>,
    mut tasks: Query<(Entity, &mut MeshingTask, &ChunkPos)>,
    surfaces: Query<(Entity, &ChunkSurface)>,
) {
    for (chunk, mut task, pos) in &mut tasks {
        let Some(groups) = block_on(future::poll_once(&mut task.0)) else {
            continue; // 还没算完，下一帧再看
        };
        commands.entity(chunk).remove::<MeshingTask>();

        for (surface, marker) in &surfaces {
            if marker.chunk == chunk {
                commands.entity(surface).despawn();
            }
        }

        let translation = pos.to_world_translation();
        let mut rendered = 0usize;
        for (voxel, mesh) in groups {
            let Some(material) = registry.get(voxel) else {
                continue;
            };
            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(translation),
                Visibility::default(),
                ChunkSurface { chunk, voxel },
            ));
            rendered += 1;
        }
        debug!("🧱 区块 {:?} 网格完成（{} 组）", pos.0, rendered);
    }
}

/// 区块卸载 → 清理它的网格实体。
///
/// 数据域只广播 [`ChunkUnloadEvent`]，表现层自己收尾（不反向依赖）。
pub fn despawn_chunk_surfaces_system(
    mut commands: Commands,
    mut unloaded: MessageReader<ChunkUnloadEvent>,
    surfaces: Query<(Entity, &ChunkSurface)>,
) {
    for event in unloaded.read() {
        for (surface, marker) in &surfaces {
            if marker.chunk == event.chunk {
                commands.entity(surface).despawn();
            }
        }
    }
}
