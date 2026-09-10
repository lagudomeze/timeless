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

/// 某个体素列的地表高度（世界体素 y）：该列最上面一块实心方块的**上方**。
///
/// 纯函数：不依赖 ECS，可直接单测；地形生成与单位/装饰贴地都调用它。
pub fn surface_height(config: &TerrainConfig, x: i32, z: i32) -> i32 {
    let amplitude = config.amplitude.max(0);
    let noise = value_noise(
        config.seed,
        x as f32 / config.scale.max(1.0),
        z as f32 / config.scale.max(1.0),
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
