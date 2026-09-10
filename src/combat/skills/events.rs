//! 技能指令消息。

use bevy::prelude::*;

/// 请求发射箭矢。
///
/// 写：[`crate::input`]（键盘只翻译）；
/// 消费：[`declare_skill_system`](super::actions::declare_skill_system)——只声明行动，
/// 到点后由射击执行器生成箭矢；消息不带目标，执行器自己查最近的敌人定朝向。
#[derive(Message, Debug, Clone, Copy)]
pub struct FireCommand;

/// 请求近战横扫。
///
/// 写：[`crate::input`]（键盘只翻译）；
/// 消费：[`declare_skill_system`](super::actions::declare_skill_system)。
#[derive(Message, Debug, Clone, Copy)]
pub struct MeleeCommand;
