//! 区块系统：固定尺寸的数据块、区块坐标与流式加载。

use bevy::prelude::*;
pub mod components;
pub mod events;
pub mod systems;

pub use components::{CHUNK_SIZE, CHUNK_VOLUME, Chunk, ChunkLoader, ChunkPinned, ChunkPos};
pub use events::{ChunkDirtyEvent, ChunkLoadEvent, ChunkUnloadEvent};
pub use systems::chunk_streaming_system;

/// 本子域系统链的位置（跨子域先后由父插件编排）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkSet;

pub mod plugin;
pub use plugin::ChunkPlugin;
