//! 破势对抗机制：同刻互击时的打断权

use bevy::prelude::*;

/// 破势值：两个攻击完全同刻且互相命中时，值高者打断对方攻击
/// （纯判定见 `domain::combat::impact_breaks`）。独立机制，不承载伤害数值。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Impact(pub u32);
