//! 体素存取子域插件：区块表资源。

use bevy::prelude::*;

use super::ChunkMap;

/// 体素的读写（按区块组织）。
///
/// 本子域只有**纯函数**（`set_voxel` / `get_voxel`）与一张区块表，
/// 没有自己的系统——所以插件只注册资源。
pub struct StoragePlugin;

impl Plugin for StoragePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChunkMap>();
    }
}
