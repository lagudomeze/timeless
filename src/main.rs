//! 可执行入口：只做插件组装。
//!
//! 引擎插件（`DefaultPlugins` / `FbxPlugin`）在这里加，业务装配在
//! [`app::GamePlugin`] 里——`main.rs` 不再认识任何具体系统。

use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy_brp_extras::BrpExtrasPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(LogPlugin {
            // wgpu 的软件渲染告警（llvmpipe）与本项目无关，压掉免得淹没有用日志。
            // 曾经这里还静音过 `icu_segmenter` / `icu_provider`，因为文档说中文断行会
            // 每帧刷 `No segmentation model for complex script`——**实测复现不出来**：
            // 打印上屏文字、跑完整局战斗都没有那行。根因是 `parley` 0.9 走的是
            // `LineSegmenter::new_for_non_complex_scripts`，`complex::select` 那条
            // 报错分支**根本不会被走到**（见 TODO.md「CJK 断行」）。所以静音已撤掉。
            filter: "wgpu=error,naga=warn".to_string(),
            ..default()
        }))
        .add_plugins(bevy::remote::RemotePlugin::default())
        .add_plugins(BrpExtrasPlugin)
        .add_plugins(app::GamePlugin)
        .run();
}
