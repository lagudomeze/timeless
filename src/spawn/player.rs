//! 玩家组装：单位零件 + 玩家专属零件。
//!
//! 驱动力来自 [`crate::input`]（键盘 → `MoveCommand` → movement 落地）；
//! 玩家额外兼任区块加载器（[`ChunkLoader`]），走哪儿加载哪儿。

use bevy::prelude::*;

use crate::combat::Faction;
use crate::movement::MoveSpeed;
use crate::world::{ChunkLoader, TerrainConfig, ground_position};

use super::unit::unit_scene;

/// 玩家场景：示例用 glTF 岩石占位（后续替换角色模型）。
pub fn player_scene(terrain: &TerrainConfig) -> impl Scene {
    let position = ground_position(terrain, 2.0, 2.0);
    let loader = ChunkLoader::default();
    bsn! {
        unit_scene(
            Faction::Player,
            position,
            3.0,
            "models/nature/rock_largeA.glb#Scene0"
        )
        MoveSpeed(5.0)
        template_value(loader)
    }
}
