//! 区块索引与体素读写 API。

use bevy::prelude::*;
pub mod ground;
pub mod interaction;
pub mod resources;
pub mod save;
pub mod systems;

pub use ground::{GROUND_REACH, ground_height, ground_height_at, ground_position_at};
pub use interaction::{BlockCommand, BlockRefused, apply_block_command_system};
pub use resources::ChunkMap;
pub use save::{
    SAVE_DIR, SAVE_FILE, SaveError, VoxelEdit, WorldEdits, WorldSave, load_world_edits_system,
    save_on_exit_system,
};
pub use systems::{get_voxel, set_voxel};

/// 本子域系统链的位置（跨子域先后由父插件编排）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StorageSet;

pub mod plugin;
pub use plugin::StoragePlugin;
