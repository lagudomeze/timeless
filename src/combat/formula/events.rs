//! 伤害消息。

use bevy::prelude::*;

use super::types::DamageType;

/// 一次伤害结算结果（已经算完护甲等减免）。
///
/// 写：伤害计算（[`apply_physical_damage_system`](super::systems::apply_physical_damage_system)
/// 等机制系统）；消费：生命值扣减、战斗日志。
#[derive(Message, Debug, Clone, Copy)]
pub struct DamageEvent {
    pub target: Entity,
    pub amount: f32,
    pub kind: DamageType,
}
