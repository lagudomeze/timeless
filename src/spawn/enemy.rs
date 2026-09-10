//! 敌人组装：单位零件 + 敌人专属零件。
//!
//! 驱动力来自 [`crate::ai`]（决策直接写 `Velocity`），与本域无关——
//! 换一种敌人只是换一组零件，不动移动 / 战斗 / 渲染。

use bevy::prelude::*;

use crate::ai::{AttackCooldown, EnemyBrain};
use crate::combat::Faction;
use crate::movement::MoveSpeed;
use crate::world::{TerrainConfig, ground_position};

use super::unit::unit_scene;

/// 敌人场景：示例用 glTF 树木占位（后续替换怪物模型）。
pub fn enemy_scene(terrain: &TerrainConfig) -> impl Scene {
    let position = ground_position(terrain, 7.0, 7.0);
    bsn! {
        unit_scene(
            Faction::Enemy,
            position,
            1.6,
            "models/nature/tree_oak.glb#Scene0"
        )
        MoveSpeed(2.0)
        EnemyBrain
        AttackCooldown
    }
}
