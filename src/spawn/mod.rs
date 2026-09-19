//! # spawn — 实体组装车间
//!
//! 「玩家」「怪物」**不是模块**，而是多个领域提供的组件在同一个实体上的组合：
//!
//! | 零件 | 提供方 |
//! | :--- | :--- |
//! | `Health` | [`crate::combat::health`] |
//! | `PhysicalDamage` / `Armor` / `HitRadius` / `AttackRange` | [`crate::combat::attributes`] |
//! | `Faction` / `Collidable` | [`crate::combat`] |
//! | `Velocity` / `MoveSpeed` / `Cell` | [`crate::movement`] |
//! | `DecisionSlot`（决策槽）/ `InputDriven`（输入归属） | [`crate::timeline`] |
//! | `EnemyBrain` / `Tactic` | [`crate::ai`] |
//! | `ChunkLoader` | [`crate::world`] |
//! | 模型 / 相机 / 装饰 / 日志 | [`crate::presentation`] |
//!
//! 本域把零件**拼装**成实体（[`player`] / [`enemy`]），并负责「开局组装」
//! （[`assembly`]）与「清场后重新组装」这类功能胶水（[`restart`]）。
//!
//! 依赖方向是单向的：
//!
//! ```text
//! spawn ──▶ combat / movement / ai / world / presentation / timeline
//! ```
//!
//! 组装层会**贴共用零件**，因此可以引用各领域（含 [`crate::timeline`] 的
//! [`DecisionSlot`](crate::timeline::DecisionSlot) 与
//! [`InputDriven`](crate::timeline::InputDriven)——前者是「能不能决策」，
//! 后者是「决策来自玩家输入」）。
//! 反向仍然禁止：**没有任何领域依赖 `spawn` 的组装逻辑**——所以改角色配置永远不会
//! 波及战斗、移动、渲染的规则。
//!
//! 唯一的例外是一条**消息**：`F5` 的触发键住在 [`crate::input`]，它写
//! [`ResetBattle`]，本域只消费——和所有其它按键一样，输入层只翻译、不执行。
//!
//! 攻击实体（箭矢、近战横扫）不在这里，它是技能的产物，工厂归
//! [`crate::combat::skills`]。

use bevy::prelude::*;

use crate::movement::Cell;
use crate::world::{TerrainConfig, ground_position};

pub mod assembly;
pub mod enemy;
pub mod player;
pub mod plugin;
pub mod restart;
pub mod unit;

pub use assembly::setup_scene;
pub use enemy::enemy_scene;
pub use player::player_scene;
pub use plugin::SpawnPlugin;
pub use restart::{ResetBattle, reset_battle_system};
pub use unit::unit_scene;

/// 某一格的**中心**在地表上的世界坐标（单位的出生点）。
///
/// 一定要用格中心：`ground_position` 只按世界坐标取高度，拿格角当出生点会让第一步
/// 走成「格角 → 邻格中心」的斜线，而且人一开始就 straddle 在两格之间。
pub(crate) fn cell_ground(terrain: &TerrainConfig, cell: Cell) -> Vec3 {
    let center = cell.center();
    ground_position(terrain, center.x, center.y)
}

/// 开局组装（Startup）：在资源预载之后跑。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssemblySet;

/// 运行期的「重新组装」（`Update`）：清场重建等功能，排在 AI / 移动 / 战斗之前。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpawnSet;
