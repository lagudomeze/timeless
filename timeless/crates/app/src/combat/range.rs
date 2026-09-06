//! 射程机制：攻击的命中距离

use bevy::prelude::*;

/// 攻击射程（网格距离，切比雪夫）。命中判定为独立的射程机制
/// （`distance <= AttackRange`，见 `domain::combat::within_range`），
/// 不感知伤害、帧速或破势。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AttackRange(pub u32);
