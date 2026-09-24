//! 体素存取子域插件：区块表资源 + 方块交互 + 存档。

use bevy::prelude::*;

use super::{ChunkMap, StorageSet};

/// 体素的读写（按区块组织）+ 方块交互 + 存档。
///
/// 读写接口（`set_voxel` / `get_voxel`）是**纯函数 + 查询**；方块交互是消费
/// [`BlockCommand`](super::BlockCommand) 的那条系统；存档把改动记进
/// [`WorldEdits`](super::WorldEdits)（**退出时才落盘**）。
pub struct StoragePlugin;

impl Plugin for StoragePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChunkMap>()
            // 方块交互：写方是 input（左键 + 修饰键），消费方是本子域
            .add_message::<super::BlockCommand>()
            // 被拒的原因：本域自己宣布，消费方是 presentation 的提示条
            .add_message::<super::BlockRefused>()
            // 存档在 `PreStartup` 装：**必须早于区块流式加载**，
            // 否则第一次生成区块时改动还没读进来，玩家改过的地形会被噪声覆盖掉。
            // 排在 `ConfigPlugin` 之后：种子来自 `TerrainConfig`，而它由配置装载。
            .add_systems(
                PreStartup,
                super::load_world_edits_system.after(crate::config::load_action_config_system),
            )
            .add_systems(Update, super::apply_block_command_system.in_set(StorageSet))
            // ⚠️ **落盘必须排在 `Last`**，不能在 `Update` 里：`AppExit` 是**帧末**
            // 由 runner 检查的（"end of an update"），而 `bevy_brp_extras` 的关闭
            // 是在它自己的系统里发的——若本系统在 `Update` 跑，可能**早于**那条消息
            // 被写出，于是这一帧读不到、下帧已经退出了，存档静默丢失。
            // 实机踩过：关掉游戏后 `saves/` 根本没建出来。
            .add_systems(Last, super::save_on_exit_system);
    }
}
