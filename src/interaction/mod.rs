//! # interaction — 鼠标拾取、点击翻译与行动预演画面
//!
//! 把「鼠标在哪里」翻译成「哪一格 / 哪个目标」，再把点击翻译成各领域的消息，
//! 同时把结果画给玩家看：
//!
//! ```text
//! 光标 ─▶ cursor_ray（相机 → 世界射线） ─▶ pick_cell（高度场步进） ─▶ HoveredCell
//!                                                                      │
//!                                             visual（高亮 / AOE / 扇形）┘
//! 点击 ─▶ pointer_command_system ─▶ MoveToCommand / UseSelectedSkill / UndoCommand
//! ```
//!
//! 域内按**"读的"与"画的"**分两半，都是同一个功能的一部分：
//!
//! | 文件 | 回答什么 | 碰什么 |
//! | :--- | :--- | :--- |
//! | [`pointer`] | 指着哪一格、点击是什么意思 | 资源 + 各领域的消息（**不产生实体**） |
//! | [`visual`] | 高亮与预演画在哪 | 网格 / 材质 / `Transform`（**只读状态**） |
//! | [`raycast`] | 一条射线打到哪一格（纯函数） | 无 Bevy 世界 |
//!
//! 本域只**读**世界（地形高度、单位所在格）与相机，对外一律走各领域的消息
//! （和 `input` 一样：只翻译，不替别人改状态）。
//!
//! **为什么不把画面搬去 `presentation`**：那样表现层就要读本域的 `HoveredCell`
//! 资源，违反 import 规则（别人的 `Resource` 不许碰）。交互的画面与交互的翻译
//! 是同一个功能的两半，留在同一个域里最省事——域内部再按职责分文件。
//!
//! 为什么单独一个域而不是塞进 `input`：拾取必须查相机与地形，而 `input` 的约定是
//! **不查询任何游戏实体**（只把设备事件翻成消息）；把它塞进去会让输入层反向依赖
//! `presentation` / `world`。

use bevy::prelude::*;

pub mod components;
pub mod events;
pub mod plugin;
pub mod pointer;
pub mod raycast;
pub mod visual;

pub use components::{AoePreview, ConePreview, HoverHighlight, HoverTint, HoveredCell};
pub use events::PointerCommand;
pub use plugin::InteractionPlugin;
pub use pointer::{hover_cell_system, pointer_command_system, update_preview_readout_system};
pub use raycast::{MAX_PICK_DISTANCE, PICK_STEP, cursor_ray, pick_cell};
pub use visual::{
    spawn_hover_highlight, spawn_preview_indicators, update_hover_highlight_system,
    update_preview_indicators_system,
};

/// 交互域在 `Update` 中的系统集（排在输入之后、时间线之前）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InteractionSet;
