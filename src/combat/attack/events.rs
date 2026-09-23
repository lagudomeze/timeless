//! 技能指令消息。
//!
//! 写：[`crate::input`]（键盘只翻译）与 [`crate::interaction`]（鼠标点目标）；
//! 消费：本域的声明系统。
//!
//! 目标格是**可选**的：键盘不给目标（`None`）时由声明系统按「最近的敌对单位」算，
//! 鼠标点某格时给 `Some(cell)` 直接打那一格——两条触发源走同一套声明逻辑。

use bevy::prelude::*;

use crate::movement::Cell;

/// 请求扔火球（`Q` 键 / 鼠标左键点单位）。
///
/// 消费：[`declare_fireball_system`](super::fireball::declare_fireball_system)——
/// 锁住目标格、消耗 2 精力、生成投射物。
///
/// `target_cell`：`None` = 打最近敌人所在的格（键盘）；`Some` = 打点中的那一格（鼠标）。
#[derive(Message, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FireCommand {
    pub target_cell: Option<Cell>,
}

/// 请求近战横扫（`E`）。
///
/// 消费：[`declare_melee_system`](super::fireball::declare_melee_system)——
/// 到点后由 `melee_action_executor_system` 朝最近敌人生成一次性横扫。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeleeCommand;
