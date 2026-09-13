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
            // 文本栈缺 CJK 分词模型时会每帧往 stderr 刷一行 `ICU4X data error: ...`：
            // 渲染本身没问题，但噪音会淹掉真正有用的日志，这里把这两个 crate 静音。
            // （真正的修法是给 `icu_segmenter` 编译进分词数据，见 TODO.md「CJK 断行」。）
            filter: "wgpu=error,naga=warn,icu_segmenter=off,icu_provider=off".to_string(),
            ..default()
        }))
        .add_plugins(bevy::remote::RemotePlugin::default())
        .add_plugins(BrpExtrasPlugin)
        .add_plugins(app::GamePlugin)
        .run();
}
