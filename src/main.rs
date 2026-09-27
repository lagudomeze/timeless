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
            //
            // `icu_provider` 的 `No segmentation model for complex script: Chinese/Japanese`
            // 也压掉——**它确实会出现**（推翻了 2026-09 那次"实测复现不出来"的结论）：
            // parley 0.9 走 `LineSegmenter::new_for_non_complex_scripts`，它的 `complex`
            // 载荷是**空的**（`ComplexPayloadsBorrowed::new()`），而断行遇到 CJK 时会去问
            // `ComplexScript::ChineseOrJapanese` 那一个分支 → 拿不到 `ja` 模型 →
            // `DataError::custom(..)` **无条件打一行 error**，然后按字回退断行。
            // 所以：**功能是对的（中文照常换行），只是每次断行刷一行日志**。
            // 想在根上消掉它得给 `icu_segmenter` 开 `auto` 特性（拉一套模型数据）——
            // 为一条日志付那个代价不值得（见 docs/backlog/clock.md 的记录）。
            filter: "wgpu=error,naga=warn,icu_provider=error".to_string(),
            ..default()
        }))
        .add_plugins(bevy::remote::RemotePlugin::default())
        .add_plugins(BrpExtrasPlugin)
        .add_plugins(app::GamePlugin)
        .run();
}
