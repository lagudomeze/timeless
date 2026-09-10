//! # ai — 敌人行为领域
//!
//! 只做「世界空间下的持续决策」：离玩家远就写自己的速度靠过去，
//! 进入攻击距离且冷却结束就生成攻击实体。AI 不感知命中结果，
//! 出手后的结算完全交给 [`crate::combat`] 流水线。

use bevy::prelude::*;

pub mod components;
pub mod plugin;
pub mod systems;

pub use components::{AttackCooldown, EnemyBrain};
pub use plugin::AiPlugin;
pub use systems::{enemy_declare_system, tick_attack_cooldown_system};

/// AI 领域在 `Update` 中的系统集（在移动之前决策，移动领域负责落地）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AiSet;
