//! 敌人组装：单位零件 + 敌人专属零件。
//!
//! 驱动力来自 [`crate::ai`]（决策槽空了就声明行动实体），与本域无关——
//! 换一种敌人只是换一组零件，不动移动 / 战斗 / 渲染。

use bevy::prelude::*;

use crate::ai::EnemyBrain;
use crate::combat::Faction;
use crate::movement::{Cell, MoveSpeed};
use crate::presentation::unit_sprite::UnitSprites;
use crate::world::TerrainConfig;

use super::cell_ground;
use super::unit::unit_scene;

/// 敌人的出生格（**多个敌人各占一格**，见 [`ENEMY_SPAWNS`]）。
pub const ENEMY_SPAWN: Cell = Cell::new(3, 3);

/// 开局出场的敌人出生格。
///
/// 两个而不是一个：**多敌人是 HUD 面板与威胁预判的前提**——只出一个敌人时，
/// "面板要画谁"这个问题根本不会出现（面板只画得下一个）。两个敌人让
/// "显示离玩家最近的那个""每个敌人都有一行"这些规则**当场可验**。
///
/// 它们**分开摆**（不是同一格）：同一格会让两个敌人的纸片重叠，
/// 而"各自独立行动"这件事在画面上也看不出来。
pub const ENEMY_SPAWNS: [Cell; 2] = [ENEMY_SPAWN, Cell::new(3, 5)];

/// 敌人场景：幽灵精灵（换贴图即可换怪物，逻辑零件不动）。
///
/// 没有独立冷却组件：敌人「多久能再决策」由它上一个动作的后摇决定
/// （见 [`crate::timeline::ActionTiming`]）。
///
/// `spawn` 是**出生格**（由组装层决定，不再是写死的常量）：
/// 多个敌人各占一格，组装层遍历 [`ENEMY_SPAWNS`] 逐个生成。
pub fn enemy_scene(terrain: &TerrainConfig, sprites: &UnitSprites, spawn: Cell) -> impl Scene {
    let position = cell_ground(terrain, spawn);
    bsn! {
        unit_scene(Faction::Enemy, position, sprites)
        MoveSpeed(2.0)
        template_value(EnemyBrain::default())
    }
}
