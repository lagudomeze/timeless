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

/// 敌人出生格（格中心出生）。
pub const ENEMY_SPAWN: Cell = Cell::new(3, 3);

/// 敌人场景：幽灵精灵（换贴图即可换怪物，逻辑零件不动）。
///
/// 没有独立冷却组件：敌人「多久能再决策」由它上一个动作的后摇决定
/// （见 [`crate::timeline::timing`]）。
pub fn enemy_scene(terrain: &TerrainConfig, sprites: &UnitSprites) -> impl Scene {
    let position = cell_ground(terrain, ENEMY_SPAWN);
    bsn! {
        unit_scene(Faction::Enemy, position, sprites)
        MoveSpeed(2.0)
        template_value(EnemyBrain::default())
    }
}
