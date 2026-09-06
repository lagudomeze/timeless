//! 伤害机制：单次命中扣减的生命值

use bevy::prelude::*;

/// 伤害数值。命中后由伤害应用系统读取并扣减目标 `Health`，
/// 与射程 / 破势 / 帧速无关，因此是独立组件而不是攻击属性包的一部分。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Damage(pub u32);
