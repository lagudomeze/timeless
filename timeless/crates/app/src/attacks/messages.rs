//! 攻击指令消息
use bevy::prelude::*;

/// 玩家请求发射：由输入系统写入，`player_fire_arrow_system` 消费并生成箭矢。
/// 消息不含目标——生成系统自行查询最近的敌人决定朝向。
#[derive(Message, Debug, Clone, Copy)]
pub struct FireCommand;
