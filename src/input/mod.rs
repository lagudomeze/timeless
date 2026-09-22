//! # input — 玩家输入源
//!
//! 本域**只做一件事**：把键盘（将来还有鼠标 / 手柄 / 面板）翻译成各领域的消息。
//! 不判断规则、不改游戏状态、不查询游戏实体——「按下 W」和「玩家加速」之间
//! 隔着一个 [`MoveCommand`](crate::movement::MoveCommand)。
//!
//! ```text
//! 键盘 ─▶ input（翻译） ─▶ MoveCommand ─────────────────▶ movement 消费（声明移动）
//!      │                ─▶ SelectSkill / UseSelectedSkill ▶ combat::skills 消费
//!      │                ─▶ ParryCommand / RollCommand ────▶ combat::defense 消费
//!      │                ─▶ PlayerTakeover / UseFocus ────────▶ timeline 消费
//!      │                ─▶ ResetBattle ────────────────────▶ spawn 消费（重置战斗）
//!      └ 鼠标 ─▶ input（翻译） ─▶ PointerCommand ─▶ interaction 消费（解释成移动 / 技能 / 撤销）
//!                            ─▶ ZoomCamera / PanCamera ─▶ presentation 消费
//! ```
//!
//! **哪个键做什么是本域的事**：空格暂停、`F5` 重置、`Q/W/E/R` 技能——各领域
//! 只收到"暂停一下"或"重置一下"这种意图，不认识 `KeyCode`。
//!
//! 唯一的例外是暂停：`pause_input_system` 要读
//! [`PauseReasons`](crate::clock::PauseReasons) 才知道"按一下是暂停还是恢复"
//! （手动暂停的闩就在那里），而"按一下切换"这个判定本来就属于输入域。
//!
//! 依赖方向：`input ──▶ movement / combat / timeline / presentation / interaction / spawn`
//! （只写它们的消息），没有领域依赖 `input`。

use bevy::prelude::*;

pub mod keyboard;
pub mod plugin;
pub mod pointer;

pub use keyboard::{
    HotkeyAction, HotkeyBinds, focus_intent_input_system, pause_input_system,
    player_help_input_system, player_move_input_system, player_skill_input_system,
    restart_input_system, skill_menu_input_system, skill_use_input_system,
};
pub use plugin::InputPlugin;
pub use pointer::{camera_pan_input_system, pointer_click_input_system};

/// 输入域在 `Update` 中的系统集（必须早于消费消息的领域）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputSet;
