//! 配置插件：`PreStartup` 装载一次（早于各域注册技能定义）。
//!
//! 用 `PreStartup` 而不是 `Startup`：各域在 `Startup` 里把技能定义交上去，
//! 而那些定义要读配置——顺序反了就会"读到的还是默认值"。

use bevy::prelude::*;

use super::load_action_config_system;

/// 动作数值配置。
pub struct ConfigPlugin;

impl Plugin for ConfigPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, load_action_config_system);
        // 热重载只在开发期编译进来（`cargo run --features hot-reload`）
        #[cfg(feature = "hot-reload")]
        super::reload::plugin(app);
    }
}
