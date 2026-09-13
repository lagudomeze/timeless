//! 交互域插件：注册资源、高亮与预演指示器，并把点击翻成各领域的消息。

use bevy::prelude::*;

use super::InteractionSet;
use super::components::{AoePreview, ConePreview, HoverHighlight, HoverTint, HoveredCell};
use super::events::PointerCommand;
use super::systems::{
    hover_cell_system, pointer_command_system, spawn_hover_highlight, spawn_preview_indicators,
    update_hover_highlight_system, update_preview_indicators_system, update_preview_readout_system,
};

/// 鼠标交互插件：悬停拾取 + 高亮 + 点击 → 消息 + 行动预演指示器。
#[derive(Debug, Default)]
pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HoveredCell>()
            // 点击消息：写方是 input 的指针系统，消费方是本域
            .add_message::<PointerCommand>()
            // 反射：BRP 能直接读「现在指着哪一格」与高亮色，排查不用猜
            .register_type::<HoveredCell>()
            .register_type::<HoverHighlight>()
            .register_type::<HoverTint>()
            .register_type::<AoePreview>()
            .register_type::<ConePreview>()
            .add_systems(Startup, (spawn_hover_highlight, spawn_preview_indicators))
            .add_systems(
                Update,
                (
                    hover_cell_system,
                    update_hover_highlight_system,
                    update_preview_indicators_system,
                    update_preview_readout_system,
                    pointer_command_system,
                )
                    .chain()
                    .in_set(InteractionSet),
            );
    }
}
