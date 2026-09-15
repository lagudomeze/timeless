//! 伤害消息。

use bevy::prelude::*;

/// 一次**已经算完减免**的伤害（纯减法，可交换）。
///
/// 写：各伤害类型的命中系统（[`apply_physical_hits_system`](super::apply_physical_hits_system)、
/// 爆炸、将来的火焰 / 毒…）；
/// 消费：[`apply_damage_system`](crate::combat::health::apply_damage_system)（唯一的扣血点）、
/// 战斗日志（[`crate::presentation::BattleLog`]）。
///
/// `source` 只为复盘 / 击杀归属存在（死亡消息要写清"谁杀的"），结算本身不看它。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageEvent {
    /// 伤害来源（攻击实体 / 施法者；环境伤害可以是 `None`）
    pub source: Option<Entity>,
    /// 被打的目标
    pub target: Entity,
    /// 扣多少血
    pub amount: i32,
}
