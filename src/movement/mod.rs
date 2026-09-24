//! # movement — 移动领域
//!
//! 只回答「实体在世界里怎么动」：速度、位移、格子坐标，以及**移动行动**
//! （载荷 + 声明 + 执行器）。不含伤害 / 命中 / 技能（那些属于 [`crate::combat`]）。
//!
//! **可行走性**在 [`rules`]：声明移动时先问"那一步迈得上去吗"，迈不上去就拒绝。
//! 判据只用**地形高度**（`world::surface_height_at`，纯函数），不查体素、
//! 也不依赖区块加载状态——那条"地形高度是纯函数"的性质是刻意保留的。
//!
//! ⚠️ **已知缺口**：玩家**自己堆的方块不影响站立高度**——单位站在噪声地表上，
//! 放三块石头也不会站上去（实测确认）。要让建造真的改变地形，得让高度查询
//! 也看体素，那会打破上面那条纯函数性质，属于单独的设计决定。
//!
//! 坐标分两层（见 [docs/timeline.md](../../docs/timeline.md) 第二节）：
//!
//! - [`Cell`]：**决策层**——行动走格、同格判定、跨格射程；
//! - `Transform` + [`Velocity`]：**结算层**——位移连续，命中 / 碰撞用真实距离。
//!
//! 位置仍然只有一份真相（Bevy 的 `Transform`，不另造 `Position` 组件），
//! [`Cell`] 是它的整数投影，只在单位停下时更新。
//!
//! 玩家只要能决策（决策槽是 `Empty`）就能动；输入是「按下的一次」，
//! 因此按一次走一格，不需要回合或窗口。

use bevy::prelude::*;

pub mod abilities;
pub mod actions;
pub mod cell;
pub mod components;
pub mod events;
pub mod plugin;
pub mod rules;
pub mod systems;

pub use actions::{
    JUMP_TIMING, JumpAction, Jumping, MOVE_TIMING, MoveAction, ROLL_TIMING, RollAction,
    declare_jump_system, declare_move_system, declare_move_to_system, ground_direction,
    jump_action_executor_system, jump_action_scene, jump_motion_system,
    move_action_executor_system, move_action_scene, roll_action_scene, step_from_axis,
};
pub use cell::{CELL_SIZE, Cell, MoveGoal};
pub use components::{MoveSpeed, Velocity};
pub use events::{JumpCommand, MoveCommand, MoveToCommand};
pub use plugin::MovementPlugin;
pub use rules::{MAX_STEP_UP, MoveRefused, can_step};
pub use systems::{DodgingOnArrival, follow_terrain_system, move_entities_system};

/// 移动领域在 `Update` 中的系统集。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MovementSet;
