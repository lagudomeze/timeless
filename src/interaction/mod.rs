//! # interaction — 鼠标交互与行动预演
//!
//! 把「鼠标在哪里」翻译成「哪一格 / 哪个目标」，再把它画给玩家看：
//!
//! ```text
//! 光标 ─▶ cursor_ray（相机 → 世界射线） ─▶ pick_cell（高度场步进） ─▶ HoveredCell
//!                                                                      │
//!                                          update_hover_highlight_system ┘
//! ```
//!
//! 本域只**读**世界（地形高度、单位所在格）与相机，只**写**自己的资源 / 高亮实体；
//! "点击 → 声明行动 / 预演"也挂在这里，但对外一律走各领域的消息
//! （和 `input` 一样：只翻译，不替别人改状态）。
//!
//! 为什么单独一个域而不是塞进 `input`：拾取必须查相机与地形，而 `input` 的约定是
//! **不查询任何游戏实体**（只把设备事件翻成消息）；把它塞进去会让输入层反向依赖
//! `presentation` / `world`。

use bevy::prelude::*;

pub mod components;
pub mod events;
pub mod plugin;
pub mod raycast;
pub mod systems;

pub use components::{AoePreview, ConePreview, HoverHighlight, HoverTint, HoveredCell};
pub use events::PointerCommand;
pub use plugin::InteractionPlugin;
pub use raycast::{MAX_PICK_DISTANCE, PICK_STEP, cursor_ray, pick_cell};
pub use systems::{
    hover_cell_system, pointer_command_system, spawn_hover_highlight, spawn_preview_indicators,
    update_hover_highlight_system, update_preview_indicators_system, update_preview_readout_system,
};

/// 交互域在 `Update` 中的系统集（排在输入之后、时间线之前）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InteractionSet;
