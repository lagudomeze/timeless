//! 体素存取子域插件：区块表资源 + 方块交互。

use bevy::prelude::*;

use super::{ChunkMap, StorageSet};

/// 体素的读写（按区块组织）+ 方块交互。
///
/// 读写接口（`set_voxel` / `get_voxel`）是**纯函数 + 查询**；方块交互是消费
/// [`BlockCommand`](super::BlockCommand) 的那条系统。
pub struct StoragePlugin;

impl Plugin for StoragePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChunkMap>()
            // 方块交互：写方是 input（左键 + 修饰键），消费方是本子域
            .add_message::<super::BlockCommand>()
            // 被拒的原因：本域自己宣布，消费方是 presentation 的提示条
            .add_message::<super::BlockRefused>()
            .add_systems(Update, super::apply_block_command_system.in_set(StorageSet));
    }
}
