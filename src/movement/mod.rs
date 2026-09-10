//! # movement — 移动领域
//!
//! 只回答「实体在世界里怎么动」：速度、位移，以及**移动行动**（载荷 + 声明 +
//! 执行器）。
//! 不含伤害 / 命中 / 技能（那些属于 [`crate::combat`]），也不认识区块数据
//! （体素碰撞是下一步：查 [`crate::world`] 的体素再决定是否位移）。
//!
//! 位置沿用 Bevy 的 `Transform`（不另造 `Position` 组件，避免两份坐标真相），
//! 本域只提供 [`Velocity`] / [`MoveSpeed`] 与位移系统。
//!
//! 规划阶段由 [`crate::timeline`] 冻结虚拟时间，玩家输入（[`crate::input`] 写
//! [`MoveCommand`]）与 AI（[`crate::ai`] 写行动实体）都只是**声明**，
//! 到点后由本域的执行器落地成 `Velocity`——移动不关心谁在推。

use bevy::prelude::*;

pub mod actions;
pub mod components;
pub mod events;
pub mod plugin;
pub mod systems;

pub use actions::{
    MoveAction, declare_move_system, move_action_executor_system, move_action_scene,
};
pub use components::{MoveSpeed, Velocity};
pub use events::MoveCommand;
pub use plugin::MovementPlugin;
pub use systems::{move_entities_system, stop_on_round_end_system};

/// 移动领域在 `Update` 中的系统集。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MovementSet;
