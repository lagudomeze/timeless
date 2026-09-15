//! 玩家组装：单位零件 + 玩家专属零件。
//!
//! 驱动力来自 [`crate::input`]（键盘 → `MoveCommand` → movement 落地）；
//! 玩家额外兼任区块加载器（[`ChunkLoader`]），走哪儿加载哪儿。

use bevy::prelude::*;

use crate::combat::Faction;
use crate::movement::{Cell, MoveSpeed};
use crate::presentation::unit_sprite::UnitSprites;
use crate::timeline::InputDriven;
use crate::world::{ChunkLoader, TerrainConfig};

use super::cell_ground;
use super::unit::unit_scene;

/// 玩家出生格（镜头也跟着它走，见 `presentation::camera`）。
///
/// 取 (1,0) 而不是 (1,1)：与敌人 (3,3) 的**格中心**间距保持约 3.6 格，
/// 和改动前（世界 (2,2) → (7,7) = 7.1 单位）基本一致。
pub const PLAYER_SPAWN: Cell = Cell::new(1, 0);

/// 玩家场景：骑士精灵（后续换一张贴图即可换角色，逻辑零件不动）。
///
/// `InputDriven` 是**输入归属**标记：时间线靠它决定「世界该停下来等谁」，
/// 反应系统靠它决定「谁被威胁」、「谁能用 Focus 换前摇」。
/// 它与 `Faction` 分工不同：`Faction` 管战斗目标过滤，两者互不替代。
pub fn player_scene(terrain: &TerrainConfig, sprites: &UnitSprites) -> impl Scene {
    let position = cell_ground(terrain, PLAYER_SPAWN);
    let loader = ChunkLoader::default();
    bsn! {
        unit_scene(Faction::Player, position, sprites)
        InputDriven
        MoveSpeed(5.0)
        template_value(loader)
    }
}
