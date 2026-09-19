//! 打断的消息。
//!
//! 伤害消息 [`DamageEvent`](crate::combat::health::DamageEvent) 跟着生命值链路住在
//! [`crate::combat::health`]；这里只留 `formula` 自己的那一条——它的生产者与消费者
//! 都在本域（命中结算触发、[`interrupt_observer`](super::systems::interrupt_observer) 当场裁决）。

use bevy::prelude::*;

/// 一次命中带来的**打断冲击**：把它自己的打断力度交给打断判定。
///
/// 用 `EntityEvent` 而不是 Message：它针对具体实体、必须即时生效
/// （Observer 里当场决定那条行动还在不在），且没有「批量广播」的语义。
///
/// 写：[`apply_physical_hits_system`](super::apply_physical_hits_system)（只有命中才谈打断）；
/// 消费：[`interrupt_observer`](super::systems::interrupt_observer)——它做掷骰对抗，
/// 赢了就把目标那条**还没到点**的行动撤掉。两边同属战斗领域，时间线不参与判定，
/// 只提供 [`ScheduledAction`](crate::timeline::ScheduledAction) 与
/// [`ActionTiming`](crate::timeline::ActionTiming) 这两样它自己的数据。
#[derive(EntityEvent, Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptEvent {
    /// 被打断的目标（`EntityEvent` 的目标实体）
    pub entity: Entity,
    /// 打断源（攻击实体 / 技能来源，日志与复盘用）
    pub source: Entity,
    /// 打断力度
    pub power: i32,
}
