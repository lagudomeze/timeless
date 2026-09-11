//! 伤害消息。

use bevy::prelude::*;

use super::types::DamageType;

/// 一次伤害结算结果（已经算完护甲等减免）。
///
/// 写：两阶段结算的落地阶段（[`crate::combat::formula::phase2_apply_system`]——
/// 阶段 1 已做完三层裁决与闪避 / 招架判定）；
/// 消费：生命值扣减（`apply_damage`）、战斗日志。
#[derive(Message, Debug, Clone, Copy)]
pub struct DamageEvent {
    pub target: Entity,
    pub amount: f32,
    pub kind: DamageType,
}
