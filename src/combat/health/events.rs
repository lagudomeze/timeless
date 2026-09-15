//! 生命值消息。

use bevy::prelude::*;

/// 目标生命归零（**同一实体只发一次**：扣血时比较扣前 / 扣后）。
///
/// 写：[`apply_damage_system`](super::systems::apply_damage_system)；
/// 消费：战斗日志（[`crate::presentation::BattleLog`]）。
///
/// 销毁不由它驱动：`despawn_dead_system` 直接看 `Health.current <= 0`，
/// 因此"谁把血扣成负的"都能被清理，不会漏。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeathEvent {
    /// 阵亡的实体
    pub entity: Entity,
    /// 击杀者（伤害来源；环境伤害可以是 `None`）
    pub killer: Option<Entity>,
}
