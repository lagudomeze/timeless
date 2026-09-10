//! 可执行入口：只做插件组装。
//!
//! 引擎插件（`DefaultPlugins` / `FbxPlugin`）在这里加，业务装配在
//! [`app::GamePlugin`] 里——`main.rs` 不再认识任何具体系统。

use bevy::prelude::*;
use bevy_ufbx::FbxPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(FbxPlugin)
        .add_plugins(app::GamePlugin)
        .run();
}
