//! # ai — 敌人行为领域
//!
//! 只做「无回合决策」：敌人的决策槽一空就选一个战术
//! 并**声明行动实体**，到点后由移动 / 技能领域的执行器落地。AI 不感知命中结果，
//! 出手后的结算完全交给 [`crate::combat`] 流水线。
//!
//! 它**不写 `Velocity`、也不改任何游戏状态**——和玩家输入一样只声明行动，
//! 这是「调度器不感知载荷」的必然结果。

use bevy::prelude::*;

pub mod components;
pub mod plugin;
pub mod systems;

pub use components::{EnemyBrain, Tactic};
pub use plugin::AiPlugin;
pub use systems::{decide_tactic_system, enemy_declare_system};

/// AI 领域在 `Update` 中的系统集（在移动之前决策，移动领域负责落地）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AiSet;
