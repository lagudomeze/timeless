//! 地面高度查询：**把"玩家放的方块"算进站立高度**。
//!
//! ## 为什么需要它（以及它打破了什么）
//!
//! 在此之前，站立高度只有一个答案：**噪声地表**（`terrain::surface_height`），
//! 一个纯函数——不查区块、不依赖加载状态。那条性质很好（单位贴地不需要区块已加载），
//! 但它有一个直接后果：**玩家放的方块完全不影响站立**。实测：连放 3 块石头，
//! `ground_position` 纹丝不动，建造在玩法上是纯装饰。
//!
//! 现在按设计决定**打破那条性质**：站立高度改为"看这一列真实的最上面一块实心方块"，
//! 因此玩家堆起来的方块真的能站上去、真的能挡路。
//!
//! ```text
//! 区块已加载 → 看真实体素（噪声地形 + 玩家改动）→ 最上面一块实心方块的上方
//! 区块未加载 → 退回噪声地表（`terrain::surface_height`），与改动前一致
//! ```
//!
//! 第二条是**兜底而不是补丁**：流式加载范围外的列本来就该按"未改动"处理
//! （玩家只能改看得见的区块），所以退回噪声是正确行为，不是权宜之计。
//!
//! ## 扫描范围
//!
//! 只扫噪声地表上下各 [`GROUND_REACH`] 个体素，而不是整列：无界扫描要遍历
//! 所有已加载区块，而这个函数的调用频率是**每单位每帧**。±8 覆盖了"往下挖"与
//! "往上堆"的合理范围，超出就按噪声值收尾（并说明这是刻意的）。

use bevy::prelude::*;

use crate::world::chunk::components::{Chunk, ChunkPos};
use crate::world::storage::ChunkMap;
use crate::world::terrain::TerrainConfig;
use crate::world::terrain::surface_height;
use crate::world::voxel::VoxelType;

/// 站立高度相对噪声地表的扫描半径（体素）。
///
/// 见模块文档「扫描范围」：这是个**代价 / 范围**的取舍，
/// 不是"世界有多高"——世界高度由区块流式加载范围决定。
pub const GROUND_REACH: i32 = 8;

/// 这一列的地面高度（世界体素 y）：最上面一块**实心方块的上方**。
///
/// 区块已加载时看真实体素（含玩家放的方块 / 挖的坑），否则退回噪声地表。
/// 这是"单位站在哪里"的**唯一答案**：贴地、位移吸附、可行走性都走它，
/// 因此三处不会各算一套。
pub fn ground_height(
    config: &TerrainConfig,
    chunk_map: &ChunkMap,
    chunks: &Query<&Chunk>,
    x: i32,
    z: i32,
) -> i32 {
    ground_height_with(config, x, z, |world| voxel_at(chunk_map, chunks, world))
}

/// [`ground_height`] 的**核心**：体素怎么读由调用方给。
///
/// 抽出来是为了两件事：
/// ① **可单测**——测试不必造 `Query`（那是系统参数类型，脱离 App 构造不出来），
///    喂一个闭包即可；
/// ② 说不清"这个函数依赖 ECS"——它其实只依赖"能读到体素"这一件事。
pub fn ground_height_with(
    config: &TerrainConfig,
    x: i32,
    z: i32,
    read_voxel: impl Fn(IVec3) -> Option<VoxelType>,
) -> i32 {
    let baseline = surface_height(config, x, z);
    // 从高往低扫：第一块实心方块的上方就是地面
    for y in (baseline - GROUND_REACH..=baseline + GROUND_REACH).rev() {
        match read_voxel(IVec3::new(x, y, z)) {
            // 区块没加载：这一列读不到，按未改动处理
            None => return baseline,
            Some(voxel) if voxel.is_opaque() => return y + 1,
            Some(_) => {}
        }
    }
    // 扫描范围内没有实心方块（挖穿了 / 堆得超出范围）：按噪声收尾
    baseline
}

/// 世界坐标 → 地面高度（`ground_height` 的浮点包装，贴地摆位用）。
pub fn ground_height_at(
    config: &TerrainConfig,
    chunk_map: &ChunkMap,
    chunks: &Query<&Chunk>,
    x: f32,
    z: f32,
) -> i32 {
    ground_height(
        config,
        chunk_map,
        chunks,
        x.floor() as i32,
        z.floor() as i32,
    )
}

/// 世界坐标 → 地面上的摆放位置（`Transform` 用）。
pub fn ground_position_at(
    config: &TerrainConfig,
    chunk_map: &ChunkMap,
    chunks: &Query<&Chunk>,
    x: f32,
    z: f32,
) -> Vec3 {
    Vec3::new(
        x,
        ground_height_at(config, chunk_map, chunks, x, z) as f32,
        z,
    )
}

/// 读一个世界体素；区块未加载时 `None`。
///
/// 与 `storage::get_voxel` 同形，但那个要 `&mut Query`（它和 `set_voxel` 同处一个
/// 文件、共用签名）；本模块只读，因此单独收一个只读查询，免得调用方
/// 为了读而拿可变查询（那会白白制造调度冲突）。
fn voxel_at(chunk_map: &ChunkMap, chunks: &Query<&Chunk>, world: IVec3) -> Option<VoxelType> {
    let pos = ChunkPos::from_voxel(world);
    let entity = chunk_map.get(pos)?;
    let chunk = chunks.get(entity).ok()?;
    Some(chunk.get(pos.local(world)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// 一个假的体素世界：坐标 → 方块。没记的位置是空气。
    #[derive(Default)]
    struct FakeWorld(HashMap<IVec3, VoxelType>);

    impl FakeWorld {
        /// 按噪声地表填一层实心地形（模拟"生成出来的世界"）。
        fn generated(config: &TerrainConfig, x: i32, z: i32, reach: i32) -> Self {
            let mut world = Self::default();
            let surface = surface_height(config, x, z);
            for y in (surface - reach)..surface {
                world.0.insert(IVec3::new(x, y, z), VoxelType::Stone);
            }
            world
        }

        fn set(&mut self, x: i32, y: i32, z: i32, voxel: VoxelType) {
            self.0.insert(IVec3::new(x, y, z), voxel);
        }

        fn read(&self, world: IVec3) -> Option<VoxelType> {
            // 表里没有 = 空气（已加载区块里的空气）
            Some(self.0.get(&world).copied().unwrap_or(VoxelType::Air))
        }
    }

    /// **没有改动时 = 原来的噪声地表**：打破不变量不等于改变默认行为。
    #[test]
    fn without_changes_the_ground_is_still_the_noise_surface() {
        let config = TerrainConfig::default();
        for (x, z) in [(0, 0), (1, 0), (4, 4)] {
            let world = FakeWorld::generated(&config, x, z, 4);
            assert_eq!(
                ground_height_with(&config, x, z, |pos| world.read(pos)),
                surface_height(&config, x, z),
                "没放东西时站立高度必须与改动前一致"
            );
        }
    }

    /// **放一块方块，那一列的地面就抬高一格**——建造真的有用。
    #[test]
    fn a_placed_block_raises_the_ground() {
        let config = TerrainConfig::default();
        let (x, z) = (2, 2);
        let baseline = surface_height(&config, x, z);
        let mut world = FakeWorld::generated(&config, x, z, 4);
        assert_eq!(
            ground_height_with(&config, x, z, |pos| world.read(pos)),
            baseline
        );

        world.set(x, baseline, z, VoxelType::Stone);
        assert_eq!(
            ground_height_with(&config, x, z, |pos| world.read(pos)),
            baseline + 1,
            "放了方块之后，站立高度应当抬高一格"
        );
    }

    /// 挖掉地表会**降低**站立高度（不是只能往上）。
    #[test]
    fn digging_lowers_the_ground() {
        let config = TerrainConfig::default();
        let (x, z) = (3, 3);
        let baseline = surface_height(&config, x, z);
        let mut world = FakeWorld::generated(&config, x, z, 4);
        // 挖掉地表那一格
        world.set(x, baseline - 1, z, VoxelType::Air);
        assert_eq!(
            ground_height_with(&config, x, z, |pos| world.read(pos)),
            baseline - 1,
            "挖掉之后地面应当降一格"
        );
    }

    /// **区块未加载 → 退回噪声**（流式范围外的列按"未改动"处理）。
    #[test]
    fn an_unloaded_column_falls_back_to_noise() {
        let config = TerrainConfig::default();
        assert_eq!(
            ground_height_with(&config, 99, 99, |_| None),
            surface_height(&config, 99, 99),
            "读不到体素时应当退回噪声地表，而不是 panic 或给 0"
        );
    }

    /// 水**不算**实心：不能站在水面上（与 `VoxelType::is_opaque` 同一判据）。
    #[test]
    fn water_is_not_solid_ground() {
        let config = TerrainConfig::default();
        let (x, z) = (5, 5);
        let baseline = surface_height(&config, x, z);
        let mut world = FakeWorld::generated(&config, x, z, 4);
        // 在地表之上倒一格水：不该把人抬起来
        world.set(x, baseline, z, VoxelType::Water);
        assert_eq!(
            ground_height_with(&config, x, z, |pos| world.read(pos)),
            baseline,
            "水不是实心，站立高度不该被它抬高"
        );
    }

    /// **堆起来的墙真的挡路**：两格之间堆高 2 格 → 走不过去。
    ///
    /// 这是"建造不再是纯装饰"的**玩法级验收**：可行走性判据与站立高度同源，
    /// 所以站立高度能看到方块，墙就挡得住人。
    #[test]
    fn a_wall_of_placed_blocks_is_not_walkable() {
        use crate::movement::rules::can_step;

        let config = TerrainConfig::default();
        let (x, z) = (1, 1);
        let baseline = surface_height(&config, x, z);

        // 先确认改动前这一段是走得通的（相邻格最多差 1）
        let mut flat = FakeWorld::generated(&config, x, z, 4);
        flat.set(x + 1, baseline - 1, z, VoxelType::Stone);
        let flat_step = can_step(
            ground_height_with(&config, x, z, |pos| flat.read(pos)),
            ground_height_with(&config, x + 1, z, |pos| flat.read(pos)),
        );
        assert!(flat_step, "平地相邻格本来就该走得通");

        // 在目标格上堆两层：高度差 2 > `MAX_STEP_UP` → 墙
        let mut walled = FakeWorld::generated(&config, x, z, 4);
        walled.set(x + 1, baseline - 1, z, VoxelType::Stone);
        walled.set(x + 1, baseline, z, VoxelType::Stone);
        walled.set(x + 1, baseline + 1, z, VoxelType::Stone);
        let blocked = can_step(
            ground_height_with(&config, x, z, |pos| walled.read(pos)),
            ground_height_with(&config, x + 1, z, |pos| walled.read(pos)),
        );
        assert!(
            !blocked,
            "堆了两层方块之后应当走不过去（差 {} 格）",
            ground_height_with(&config, x + 1, z, |pos| walled.read(pos)) - baseline
        );
    }

    /// 扫描范围之外（堆得太高 / 挖穿）按噪声收尾，而不是无限找。
    #[test]
    fn beyond_the_reach_it_falls_back_rather_than_scanning_forever() {
        let config = TerrainConfig::default();
        let (x, z) = (6, 6);
        let baseline = surface_height(&config, x, z);
        let mut world = FakeWorld::generated(&config, x, z, 4);
        // 挖空整个可见范围：读不到实心方块 → 退回噪声
        for y in (baseline - GROUND_REACH)..=(baseline + GROUND_REACH) {
            world.set(x, y, z, VoxelType::Air);
        }
        assert_eq!(
            ground_height_with(&config, x, z, |pos| world.read(pos)),
            baseline,
            "范围内没有实心方块时按噪声收尾（刻意的：不做无界扫描）"
        );
    }
}
