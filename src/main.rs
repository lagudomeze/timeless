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
            // 文本栈每帧往 stderr 刷一行 `ICU4X data error: No segmentation model
            // for complex script`：那是 `icu_segmenter` 的**附加词模型**没编译进来，
            // 中文的断行与分词其实正常（走的是 `LineSegmenter` 自带的 `cjdict`）。
            // 渲染没问题，但噪音会淹掉真正有用的日志，这里把这两个 crate 静音
            // （见 TODO.md「CJK 断行」）。
            filter: "wgpu=error,naga=warn,icu_segmenter=off,icu_provider=off".to_string(),
            ..default()
        }))
        .add_plugins(bevy::remote::RemotePlugin::default())
        .add_plugins(BrpExtrasPlugin)
        .add_plugins(app::GamePlugin)
        .run();
}
