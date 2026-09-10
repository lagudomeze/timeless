//! 生命值消息。

use bevy::prelude::*;

/// 一次血量变化请求（负数为扣血，正数为治疗）。
///
/// 写：伤害结算（[`DamageEvent`](crate::combat::DamageEvent) 之外的来源也可直接用，
/// 例如中毒、再生）；消费：[`apply_damage`](super::systems::apply_damage)。
#[derive(Message, Debug, Clone, Copy)]
pub struct ModifyHealthEvent {
    pub target: Entity,
    pub amount: f32,
}

/// 目标生命归零。
///
/// 写：[`apply_damage`](super::systems::apply_damage)；
/// 消费：[`despawn_dead_system`](super::systems::despawn_dead_system)（销毁实体）、
/// 战斗日志（[`crate::scene::BattleLog`]）。
#[derive(Message, Debug, Clone, Copy)]
pub struct DeathEvent {
    pub entity: Entity,
}
