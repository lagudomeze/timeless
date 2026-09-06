//! 帧速机制：出招速度（越小越快）

use bevy::prelude::*;

/// 出招帧数：数值越小越快。前摇时长 = `frame × TICK_MS`；
/// 两个攻击完全同刻命中时，帧小者先中（纯判定见 `domain::combat::earlier_side`）。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AttackFrame(pub u32);
