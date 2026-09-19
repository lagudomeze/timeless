//! 地形生成：纯函数（可单测） + 一个「区块加载 → 填数据」的系统。

use bevy::prelude::*;

use crate::world::chunk::components::{CHUNK_SIZE, Chunk, ChunkPos};
use crate::world::chunk::events::{ChunkDirtyEvent, ChunkLoadEvent};
use crate::world::voxel::VoxelType;

use super::resources::TerrainConfig;

/// 整点哈希 → 0..1 的伪随机值（值噪声的格点值）。
fn hash01(seed: u32, x: i32, z: i32) -> f32 {
    let mut hash =
        seed ^ (x as u32).wrapping_mul(0x9E37_79B9) ^ (z as u32).wrapping_mul(0x85EB_CA6B);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x2545_F491);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(0x27D4_EB2D);
    hash ^= hash >> 16;
    (hash & 0xFFFF) as f32 / 65_535.0
}

/// 双线性插值 + smoothstep 的值噪声，返回 0..1。
fn value_noise(seed: u32, x: f32, z: f32) -> f32 {
    let (x0, z0) = (x.floor(), z.floor());
    let (ix, iz) = (x0 as i32, z0 as i32);
    let (tx, tz) = (x - x0, z - z0);

    let v00 = hash01(seed, ix, iz);
    let v10 = hash01(seed, ix + 1, iz);
    let v01 = hash01(seed, ix, iz + 1);
    let v11 = hash01(seed, ix + 1, iz + 1);

    let sx = tx * tx * (3.0 - 2.0 * tx);
    let sz = tz * tz * (3.0 - 2.0 * tz);
    let top = v00 + (v10 - v00) * sx;
    let bottom = v01 + (v11 - v01) * sx;
    top + (bottom - top) * sz
}

/// 地形高度的量化粒度（体素）：`TERRAIN_CELL × TERRAIN_CELL` 个体素列共享一个高度。
///
/// **必须等于决策格 `Cell` 的边长**（`movement::CELL_SIZE`，有测试守着）。
///
/// 为什么地形要跟着决策格走：决策层"一格一步"，单位与装饰都摆在**格中心**——
/// 而格中心（CELL_SIZE = 2 时是奇数世界坐标）正好落在体素**边界**上。如果地形按
/// 单个体素列起伏，格中心两侧就可能差一格高，footprint 稍大的对象（树、单位纸片）
/// 就会有一半陷进邻居方块里：看上去就是"树长在地面下、人跟地面重叠"。
/// 量化到格之后，一格 = 一块 2×2 的平地，台阶只出现在格与格的边界上。
pub const TERRAIN_CELL: i32 = 2;

/// 某个体素列的地表高度（世界体素 y）：该列最上面一块实心方块的**上方**。
///
/// 纯函数：不依赖 ECS，可直接单测；地形生成与单位/装饰贴地都调用它。
/// 采样点取**所在格的中心**，因此整格同高（见 [`TERRAIN_CELL`]）。
pub fn surface_height(config: &TerrainConfig, x: i32, z: i32) -> i32 {
    let amplitude = config.amplitude.max(0);
    let cell_center =
        |v: i32| (v.div_euclid(TERRAIN_CELL) * TERRAIN_CELL + TERRAIN_CELL / 2) as f32;
    let noise = value_noise(
        config.seed,
        cell_center(x) / config.scale.max(1.0),
        cell_center(z) / config.scale.max(1.0),
    );
    let steps = (noise * (amplitude + 1) as f32).floor() as i32;
    config.base_height - steps.clamp(0, amplitude)
}

/// 世界坐标 → 该列地表高度（贴地摆放用的便捷包装）。
pub fn surface_height_at(config: &TerrainConfig, x: f32, z: f32) -> i32 {
    surface_height(config, x.floor() as i32, z.floor() as i32)
}

/// 世界坐标 → 地表上的摆放位置（单位 / 装饰贴地用）。
pub fn ground_position(config: &TerrainConfig, x: f32, z: f32) -> Vec3 {
    Vec3::new(x, surface_height_at(config, x, z) as f32, z)
}

/// 按配置生成一个区块的地形数据（纯函数，不需要 ECS）。
///
/// 分层：地表为草、往下数层泥土、更深处石头；地表以上为空气。
pub fn generate_chunk_terrain(config: &TerrainConfig, pos: ChunkPos, chunk: &mut Chunk) {
    let origin = pos.origin();
    for z in 0..CHUNK_SIZE as u32 {
        for x in 0..CHUNK_SIZE as u32 {
            let height = surface_height(config, origin.x + x as i32, origin.z + z as i32);
            for y in 0..CHUNK_SIZE as u32 {
                // depth < 0 表示在地表之上；0 是最上面一层实心方块
                let depth = height - 1 - (origin.y + y as i32);
                let voxel = if depth < 0 {
                    VoxelType::Air
                } else {
                    match depth {
                        0 => VoxelType::Grass,
                        1..=3 => VoxelType::Dirt,
                        _ => VoxelType::Stone,
                    }
                };
                chunk.set(UVec3::new(x, y, z), voxel);
            }
        }
    }
}

/// 区块加载 → 生成地形数据 → 标记为脏（渲染层据此建网格）。
///
/// 数据生成是纯计算，不涉及网格与材质；跨域只发 [`ChunkDirtyEvent`]。
pub fn generate_terrain_system(
    config: Res<TerrainConfig>,
    mut chunks: Query<(&ChunkPos, &mut Chunk)>,
    mut loads: MessageReader<ChunkLoadEvent>,
    mut dirty: MessageWriter<ChunkDirtyEvent>,
) {
    for event in loads.read() {
        let Ok((pos, mut chunk)) = chunks.get_mut(event.chunk) else {
            continue;
        };
        generate_chunk_terrain(&config, *pos, &mut chunk);
        dirty.write(ChunkDirtyEvent { chunk: event.chunk });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 地形量化粒度必须与决策格一致——差一格就会出现"树长在地面下、人陷进地里"。
    #[test]
    fn terrain_quantisation_matches_the_decision_cell_size() {
        assert_eq!(
            TERRAIN_CELL as f32,
            crate::movement::CELL_SIZE,
            "地形必须与 `Cell` 同粒度：单位与装饰都摆在格中心（体素边界）上"
        );
    }

    /// 一格内部的体素列必须同高：格中心两侧差一格，footprint 稍大的对象就会被埋一半。
    #[test]
    fn every_voxel_column_inside_a_cell_shares_one_height() {
        let config = TerrainConfig::default();
        for cell_x in -3..6 {
            for cell_z in -3..6 {
                let base = surface_height(&config, cell_x * TERRAIN_CELL, cell_z * TERRAIN_CELL);
                for dx in 0..TERRAIN_CELL {
                    for dz in 0..TERRAIN_CELL {
                        let x = cell_x * TERRAIN_CELL + dx;
                        let z = cell_z * TERRAIN_CELL + dz;
                        assert_eq!(
                            surface_height(&config, x, z),
                            base,
                            "格 ({cell_x},{cell_z}) 内的 ({x},{z}) 与格角不同高"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn terrain_is_deterministic_for_the_same_seed_and_position() {
        let config = TerrainConfig::default();
        let a = surface_height(&config, 3, -7);
        let b = surface_height(&config, 3, -7);
        assert_eq!(a, b, "同 seed + 同坐标必须得到同一高度（区块无缝）");

        let other = TerrainConfig {
            seed: config.seed + 1,
            ..config
        };
        let changed =
            (0..64).any(|x| surface_height(&config, x, 0) != surface_height(&other, x, 0));
        assert!(changed, "换种子应当换地形");
    }

    #[test]
    fn surface_height_stays_within_the_configured_band() {
        let config = TerrainConfig {
            base_height: 5,
            amplitude: 3,
            ..TerrainConfig::default()
        };
        for x in -20..20 {
            for z in -20..20 {
                let height = surface_height(&config, x, z);
                assert!(
                    (config.base_height - config.amplitude..=config.base_height).contains(&height),
                    "地表高度 {height} 应在配置的起伏带内"
                );
            }
        }
    }

    #[test]
    fn generated_chunk_layers_grass_dirt_and_stone_under_the_surface() {
        let config = TerrainConfig::default();
        let pos = ChunkPos(IVec3::new(0, -1, 0));
        let mut chunk = Chunk::empty();
        generate_chunk_terrain(&config, pos, &mut chunk);

        let origin = pos.origin();
        for (x, z) in [(0, 0), (17, 5), (31, 31)] {
            let height = surface_height(&config, origin.x + x, origin.z + z);
            let top_local_y = (height - 1 - origin.y) as u32;
            assert_eq!(
                chunk.get(UVec3::new(x as u32, top_local_y, z as u32)),
                VoxelType::Grass,
                "最上面一层实心方块应为草"
            );
            if (top_local_y as usize) < CHUNK_SIZE - 1 {
                assert_eq!(
                    chunk.get(UVec3::new(x as u32, top_local_y + 1, z as u32)),
                    VoxelType::Air,
                    "地表之上应为空气"
                );
            }
            assert_eq!(
                chunk.get(UVec3::new(x as u32, top_local_y - 1, z as u32)),
                VoxelType::Dirt
            );
        }
    }

    #[test]
    fn ground_position_sits_on_the_surface() {
        let config = TerrainConfig::default();
        let position = ground_position(&config, 2.5, 7.5);
        assert_eq!(position.x, 2.5);
        assert_eq!(position.z, 7.5);
        assert_eq!(
            position.y as i32,
            surface_height(&config, 2, 7),
            "贴地位置应当正好落在地表高度上"
        );
    }
}
