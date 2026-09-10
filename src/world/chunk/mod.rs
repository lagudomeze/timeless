//! 区块系统：固定尺寸的数据块、区块坐标与流式加载。

pub mod components;
pub mod events;
pub mod systems;

pub use components::{CHUNK_SIZE, CHUNK_VOLUME, Chunk, ChunkLoader, ChunkPinned, ChunkPos};
pub use events::{ChunkDirtyEvent, ChunkLoadEvent, ChunkUnloadEvent};
pub use systems::chunk_streaming_system;
