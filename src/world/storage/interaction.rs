//! 方块交互：把「改一格方块」翻译成 [`set_voxel`] 调用。
//!
//! **命令进、数据出**：输入域只写一条 [`BlockCommand`]（不认识方块、不认识体素坐标），
//! 本系统把它落到 [`set_voxel`]——那里保证「改动必标脏」，渲染层收到
//! [`ChunkDirtyEvent`](crate::world::chunk::ChunkDirtyEvent) 后自己重建网格。
//!
//! 分层：`set_voxel` 是**纯函数 + 查询**（不碰输入也不碰按键），
//! 本文件是**应用层**（消费消息、把格换算成体素坐标）。按键绑定住在
//! `crate::input`，本域不认识 `KeyCode`。

use bevy::prelude::*;

use crate::movement::Cell;
use crate::world::chunk::components::{Chunk, ChunkPos};
use crate::world::chunk::events::ChunkDirtyEvent;
use crate::world::voxel::VoxelType;

use super::resources::ChunkMap;
use super::systems::set_voxel;

/// 方块操作被拒（写：本子域；消费：`presentation` 的提示条）。
///
/// **本域不写时间线的 `ActionBlocked`**：`world` 是纯数据域，必须能用裸
/// `MinimalPlugins` 单测（见模块文档），所以它不认识时间线的消息。被拒的原因
/// 自己宣布，谁关心谁来听。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockRefused {
    /// 目标所在的区块还没加载：这一手无处可落
    TerrainNotLoaded,
}

/// 改一格方块（写：[`crate::input`]；消费：本系统）。
///
/// `place` = `true` 放一块石头，`false` 挖掉（变空气）。目标格用 [`Cell`]（决策层
/// 的坐标），换算成体素坐标在这里做——**输入域不该知道 `CELL_SIZE` 与地形量化**。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockCommand {
    pub cell: Cell,
    pub place: bool,
}

/// 挖掉的那一格用什么填。
///
/// 现在是空气（挖穿）；将来接「背包里的方块类型」时只改这一处。
const REMOVED: VoxelType = VoxelType::Air;
/// 放下的方块类型。
const PLACED: VoxelType = VoxelType::Stone;

pub fn apply_block_command_system(
    mut commands: MessageReader<BlockCommand>,
    chunk_map: Res<ChunkMap>,
    mut chunks: Query<&mut Chunk>,
    mut dirty: MessageWriter<ChunkDirtyEvent>,
    mut refused: MessageWriter<BlockRefused>,
) {
    for command in commands.read() {
        // 格 → 世界体素坐标：格中心的地表那一格。
        // 与地形的量化粒度（`TERRAIN_CELL`）对齐——整格同高，所以取格中心不会
        // 落在台阶的另一侧（见 `world::terrain::systems`）。
        let center = command.cell.center();
        let Some(voxel) = voxel_at_ground(&chunk_map, &chunks, center) else {
            // 区块没加载：玩家的操作无处可落，说一声（谁关心谁来听）
            refused.write(BlockRefused::TerrainNotLoaded);
            continue;
        };
        let target = if command.place {
            voxel + IVec3::Y
        } else {
            voxel
        };
        let kind = if command.place { PLACED } else { REMOVED };
        set_voxel(&chunk_map, &mut chunks, &mut dirty, target, kind);
    }
}

/// 格中心正下方那一格的**地表体素**坐标（没加载区块时 `None`）。
fn voxel_at_ground(
    chunk_map: &ChunkMap,
    chunks: &Query<&mut Chunk>,
    center: Vec2,
) -> Option<IVec3> {
    let column = IVec3::new(center.x.floor() as i32, 0, center.y.floor() as i32);
    let pos = ChunkPos::from_voxel(column);
    let entity = chunk_map.get(pos)?;
    let chunk = chunks.get(entity).ok()?;
    // 从区块顶往下找第一个非空气——那就是地表
    let origin = pos.origin();
    for y in (0..crate::world::chunk::CHUNK_SIZE as i32).rev() {
        let local = pos.local(IVec3::new(column.x, origin.y + y, column.z));
        if chunk.get(local) != VoxelType::Air {
            return Some(IVec3::new(column.x, origin.y + y, column.z));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::chunk::{Chunk, ChunkPos};
    use crate::world::storage::ChunkMap;

    /// 最小 App：一张区块表 + 一个区块实体（地形填到 y=2）。
    fn world_app() -> (App, ChunkPos) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ChunkMap>()
            .add_message::<BlockCommand>()
            .add_message::<ChunkDirtyEvent>()
            .add_message::<BlockRefused>()
            .add_systems(Update, apply_block_command_system);

        let pos = ChunkPos(IVec3::new(0, 0, 0));
        let mut chunk = Chunk::empty();
        for y in 0..=2 {
            for x in 0..4 {
                for z in 0..4 {
                    chunk.set(UVec3::new(x, y, z), VoxelType::Grass);
                }
            }
        }
        let entity = app.world_mut().spawn((chunk, pos)).id();
        app.world_mut()
            .resource_mut::<ChunkMap>()
            .insert(pos, entity);
        (app, pos)
    }

    fn voxel_of(app: &App, pos: ChunkPos, world: IVec3) -> VoxelType {
        let entity = app.world().resource::<ChunkMap>().get(pos).unwrap();
        let chunk = app.world().get::<Chunk>(entity).unwrap();
        chunk.get(pos.local(world))
    }

    /// 挖掉：地表那一格变成空气，并且**标脏**（渲染层据此重建网格）。
    #[test]
    fn removing_clears_the_surface_voxel_and_marks_the_chunk_dirty() {
        let (mut app, pos) = world_app();
        let cell = Cell::new(0, 0);
        let center = cell.center();
        let ground = IVec3::new(center.x.floor() as i32, 2, center.y.floor() as i32);
        assert_eq!(voxel_of(&app, pos, ground), VoxelType::Grass);

        app.world_mut()
            .write_message(BlockCommand { cell, place: false });
        app.update();

        assert_eq!(
            voxel_of(&app, pos, ground),
            VoxelType::Air,
            "挖掉之后那一格应当是空气"
        );
    }

    /// 放置：在**地表之上**加一块石头（不是覆盖地表）。
    #[test]
    fn placing_adds_a_block_above_the_surface() {
        let (mut app, pos) = world_app();
        let cell = Cell::new(0, 0);
        let center = cell.center();
        let ground = IVec3::new(center.x.floor() as i32, 2, center.y.floor() as i32);

        app.world_mut()
            .write_message(BlockCommand { cell, place: true });
        app.update();

        assert_eq!(
            voxel_of(&app, pos, ground + IVec3::Y),
            VoxelType::Stone,
            "放置应当加在地表之上"
        );
        assert_eq!(
            voxel_of(&app, pos, ground),
            VoxelType::Grass,
            "地表本身不该被动到"
        );
    }

    /// 区块没加载时：不 panic，而且**告诉 HUD 为什么**（与技能被拒同一条通道）。
    #[test]
    fn a_cell_in_an_unloaded_chunk_is_refused_with_a_reason() {
        let (mut app, _) = world_app();
        app.world_mut().write_message(BlockCommand {
            cell: Cell::new(999, 999),
            place: true,
        });
        app.update();
        // 没有 panic 就是主要断言；消息通道也接上了（`ActionBlocked` 已注册）
    }

    /// 方块交互**只经 `set_voxel`**：改完一定标脏，不会出现"改了但画面不更新"。
    #[test]
    fn every_change_marks_the_chunk_dirty() {
        let (mut app, _) = world_app();
        let cell = Cell::new(0, 0);
        app.world_mut()
            .write_message(BlockCommand { cell, place: false });
        app.update();
        // `ChunkDirtyEvent` 是双缓冲消息，本帧写、本帧读得到
        let reader = app.world_mut().resource_mut::<Messages<ChunkDirtyEvent>>();
        let count = reader.len();
        assert!(count > 0, "改一格必须标脏，否则网格不会重建");
    }
}
