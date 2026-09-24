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
    edits: Option<Res<crate::world::storage::WorldEdits>>,
    mut chunks: Query<(&ChunkPos, &mut Chunk)>,
    mut loads: MessageReader<ChunkLoadEvent>,
    mut dirty: MessageWriter<ChunkDirtyEvent>,
) {
    for event in loads.read() {
        let Ok((pos, mut chunk)) = chunks.get_mut(event.chunk) else {
            continue;
        };
        generate_chunk_terrain(&config, *pos, &mut chunk);
        // 存档里的改动**盖在噪声地形之上**：地形是"算得出来"的那一层，
        // 改动是"玩家留下的"那一层，两者叠加才是世界该有的样子（见 `storage::save`）。
        // `Option`：只装地形子域的轻量单测没有存档资源，那时就是"没有改动"。
        if let Some(edits) = edits.as_deref() {
            apply_edits_to_chunk(edits, *pos, &mut chunk);
        }
        dirty.write(ChunkDirtyEvent { chunk: event.chunk });
    }
}

/// 把落在 `pos` 这个区块里的改动盖上去（纯函数 + 查询，可单测）。
///
/// 逐条判断"这一条在不在这个区块里"：改动条数是个位 / 十位量级，而每帧只在
/// **区块加载**时跑一次，直扫比建索引简单得多（够用就不加机制）。
fn apply_edits_to_chunk(
    edits: &crate::world::storage::WorldEdits,
    pos: ChunkPos,
    chunk: &mut Chunk,
) {
    let origin = pos.origin();
    let size = CHUNK_SIZE as i32;
    for edit in edits.iter() {
        let world = edit.position();
        // 落在区块外的改动跳过（世界体素 → 区块坐标由 `ChunkPos` 回答）
        if ChunkPos::from_voxel(world) != pos {
            continue;
        }
        let local = world - origin;
        if local.x < 0 || local.y < 0 || local.z < 0 {
            continue; // 理论上不可达（from_voxel 已经保证），防御性跳过
        }
        if local.x >= size || local.y >= size || local.z >= size {
            continue;
        }
        let Some(kind) = edit.kind() else {
            continue; // 存档里的方块类型不认识（旧档 / 手改坏了）：跳过这一条
        };
        chunk.set(local.as_uvec3(), kind);
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

    /// **存档改动真的盖回了地形上**：加载时先按噪声生成、再把改动覆盖上去。
    ///
    /// 这是"只存改动"这条路子的**端点验收**（`storage::save` 的模块文档解释了
    /// 为什么不存整块地形）：单测过 `WorldSave` 的往返、也过 `set_voxel` 的写入，
    /// 但那是两半；真正要成立的是"**重开一次游戏，那块石头还在**"。
    #[test]
    fn saved_edits_are_laid_over_the_generated_terrain() {
        use crate::world::storage::{VoxelEdit, WorldEdits};

        let config = TerrainConfig::default();
        let pos = ChunkPos(IVec3::new(0, -1, 0));
        let origin = pos.origin();
        // 在区块里挑一格地表（生成出来一定有方块），把它改成石头并记进"存档"
        let surface = surface_height(&config, origin.x + 5, origin.z + 7);
        let target = IVec3::new(origin.x + 5, surface - 1, origin.z + 7);

        let mut edits = WorldEdits::default();
        edits.record(VoxelEdit::new(target, VoxelType::Stone));

        // 1) 没有存档改动时，那一格是生成出来的草 / 土
        let mut plain = Chunk::empty();
        generate_chunk_terrain(&config, pos, &mut plain);
        let generated = plain.get(pos.local(target));
        assert_ne!(generated, VoxelType::Stone, "先确认这一格本来不是石头");

        // 2) 有改动时，同一格被盖成石头
        let mut saved = Chunk::empty();
        generate_chunk_terrain(&config, pos, &mut saved);
        apply_edits_to_chunk(&edits, pos, &mut saved);
        assert_eq!(
            saved.get(pos.local(target)),
            VoxelType::Stone,
            "存档里的改动必须盖在噪声地形之上"
        );

        // 3) 区块**外面**的改动不能污染这一块
        let elsewhere = WorldEdits::default();
        let mut untouched = Chunk::empty();
        generate_chunk_terrain(&config, pos, &mut untouched);
        apply_edits_to_chunk(&elsewhere, pos, &mut untouched);
        assert_eq!(
            untouched.get(pos.local(target)),
            generated,
            "别的区块的改动不该动到这一块"
        );
    }

    /// 存档里的坐标落在**另一个区块**时，这一块一个格子都不该被动。
    #[test]
    fn an_edit_in_another_chunk_is_ignored() {
        use crate::world::storage::{VoxelEdit, WorldEdits};

        let config = TerrainConfig::default();
        let pos = ChunkPos(IVec3::new(0, 0, 0));
        // 一条明确落在别处的改动（区块 (9,9,9) 内部）
        let far = ChunkPos(IVec3::new(9, 9, 9)).origin() + IVec3::new(1, 1, 1);
        let mut edits = WorldEdits::default();
        edits.record(VoxelEdit::new(far, VoxelType::Stone));

        let mut chunk = Chunk::empty();
        generate_chunk_terrain(&config, pos, &mut chunk);
        let before = chunk.voxels.clone();
        apply_edits_to_chunk(&edits, pos, &mut chunk);

        assert_eq!(chunk.voxels, before, "别处的改动不该写进这一块");
    }

    /// 存档里有一个**不认识的方块名**（旧档 / 手改坏了）时跳过它，不 panic。
    #[test]
    fn an_unknown_voxel_name_in_the_save_is_skipped() {
        use crate::world::storage::{VoxelEdit, WorldEdits, WorldSave};

        let config = TerrainConfig::default();
        let pos = ChunkPos(IVec3::new(0, -1, 0));
        let target = pos.origin() + IVec3::new(2, 3, 4);
        let mut edits = WorldEdits::from_save(WorldSave {
            seed: 0,
            edits: vec![VoxelEdit {
                x: target.x,
                y: target.y,
                z: target.z,
                voxel: "unobtainium".to_string(),
            }],
        });
        // 再叠一条能认的，确认"跳过坏的、保留好的"
        let good = pos.origin() + IVec3::new(6, 3, 8);
        edits.record(VoxelEdit::new(good, VoxelType::Wood));

        let mut chunk = Chunk::empty();
        generate_chunk_terrain(&config, pos, &mut chunk);
        let generated = chunk.voxels.clone();
        apply_edits_to_chunk(&edits, pos, &mut chunk);

        assert_eq!(
            chunk.get(pos.local(target)),
            generated[Chunk::index(pos.local(target))],
            "不认识的类型不能改动地形"
        );
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
