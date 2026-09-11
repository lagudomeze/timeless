//! 敌人组装：单位零件 + 敌人专属零件。
//!
//! 驱动力来自 [`crate::ai`]（有 `Ready` 就声明行动实体），与本域无关——
//! 换一种敌人只是换一组零件，不动移动 / 战斗 / 渲染。

use bevy::prelude::*;

use crate::ai::EnemyBrain;
use crate::combat::Faction;
use crate::movement::MoveSpeed;
use crate::world::{TerrainConfig, ground_position};

use super::unit::unit_scene;

/// 敌人场景：示例用 glTF 树木占位（后续替换怪物模型）。
///
/// 没有独立冷却组件：敌人「多久能再决策」由它上一个动作的后摇决定
/// （见 [`crate::timeline::timing`]）。
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
        template_value(EnemyBrain::default())
    }
}
