//! 区块流式子域插件：区块消息与流式加载系统。

use bevy::prelude::*;

use super::{ChunkDirtyEvent, ChunkLoadEvent, ChunkSet, ChunkUnloadEvent, chunk_streaming_system};

/// 区块的增删（按加载器覆盖范围）。
pub struct ChunkPlugin;

impl Plugin for ChunkPlugin {
    fn build(&self, app: &mut App) {
        app
            // 区块消息：写方是本域，消费方是 `voxel_render`（表现层只读数据）
            .add_message::<ChunkLoadEvent>()
            .add_message::<ChunkUnloadEvent>()
            .add_message::<ChunkDirtyEvent>()
            .add_systems(Update, chunk_streaming_system.in_set(ChunkSet));
    }
}
