//! 技能指令消息。
//!
//! 写：[`crate::input`]（键盘只翻译）；消费：本域的声明系统。
//! 消息都不带目标——落点由声明系统按「最近的敌对单位」自己算
//! （火球锁它的格，近战朝它的方向扫）。

use bevy::prelude::*;

/// 请求扔火球（`Q`）。
///
/// 消费：[`declare_fireball_system`](super::fireball::declare_fireball_system)——
/// 锁住最近敌人所在的格、消耗 2 精力、生成投射物。
#[derive(Message, Debug, Clone, Copy)]
pub struct FireCommand;

/// 请求近战横扫（`E`）。
///
/// 消费：[`declare_melee_system`](super::fireball::declare_melee_system)——
/// 到点后由 `melee_action_executor_system` 朝最近敌人生成一次性横扫。
#[derive(Message, Debug, Clone, Copy)]
pub struct MeleeCommand;
