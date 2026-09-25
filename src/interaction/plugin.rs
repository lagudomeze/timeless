//! 交互域插件：拾取悬停格、画高亮与预演，并把点击翻成各领域的消息。
//!
//! 系统顺序（`InteractionSet` 内 `.chain()`）：先算 UI 门控（写 [`PointerOverUi`]），
//! 再拾取（写 `HoveredCell`），再画高亮 / 预演（读它），然后发预演读数、
//! 最后翻译点击——**同一帧内**，玩家看到的画面与点击的解释用的是同一个悬停格。
//!
//! **UI 门控排在最前**：指针压在 HUD 上时，这一帧既不拾取（`HoveredCell = None`），
//! 也不翻译任何点击——否则点一下日志标题会顺手把人走一格。判据见
//! [`ui_capture`](super::ui_capture)。

use bevy::prelude::*;

use super::InteractionSet;
use super::components::{AoePreview, ConePreview, HoverHighlight, HoverTint, HoveredCell};
use super::events::PointerCommand;
use super::pointer::{hover_cell_system, pointer_command_system, update_preview_readout_system};
use super::ui_capture::{PointerOverUi, track_pointer_over_ui_system};
use super::visual::{
    spawn_hover_highlight, spawn_preview_indicators, update_hover_highlight_system,
    update_preview_indicators_system,
};

/// 鼠标交互插件：悬停拾取 + 高亮 + 点击 → 消息 + 行动预演指示器。
#[derive(Debug, Default)]
pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HoveredCell>()
            // 指针有没有被 UI 吃掉：拾取与点击都门控在它上
            .init_resource::<PointerOverUi>()
            // 点击消息：写方是 input 的指针系统，消费方是本域
            .add_message::<PointerCommand>()
            // 反射：BRP 能直接读「现在指着哪一格」与高亮色，排查不用猜
            .register_type::<HoveredCell>()
            .register_type::<PointerOverUi>()
            .register_type::<HoverHighlight>()
            .register_type::<HoverTint>()
            .register_type::<AoePreview>()
            .register_type::<ConePreview>()
            .add_systems(Startup, (spawn_hover_highlight, spawn_preview_indicators))
            .add_systems(
                Update,
                (
                    track_pointer_over_ui_system,
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
