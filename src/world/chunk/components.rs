//! 区块组件：体素数据块、区块坐标与加载器。

use bevy::prelude::*;

use crate::world::voxel::VoxelType;

/// 区块边长（体素）。
pub const CHUNK_SIZE: usize = 32;

/// 单个区块的体素总数。
pub const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

/// 区块实体：持有一整块体素数据。
///
/// 纯数据——不含任何网格 / 材质句柄，那些属于表现层（`voxel_render`）。
#[derive(Component, Debug, Clone)]
pub struct Chunk {
    /// 体素数组，下标见 [`Chunk::index`]
    pub voxels: Box<[VoxelType; CHUNK_VOLUME]>,
}

impl Default for Chunk {
    fn default() -> Self {
        Self::empty()
    }
}

impl Chunk {
    /// 全空气区块。
    pub fn empty() -> Self {
        Self {
            voxels: Box::new([VoxelType::Air; CHUNK_VOLUME]),
        }
    }

    /// 区块内局部坐标 → 数组下标（x 最快、z 最慢）。
    ///
    /// 坐标越界属于调用方错误，直接 panic（debug 断言便于定位）。
    pub fn index(local: UVec3) -> usize {
        debug_assert!(
            local.x < CHUNK_SIZE as u32
                && local.y < CHUNK_SIZE as u32
                && local.z < CHUNK_SIZE as u32,
            "区块局部坐标越界：{local:?}"
        );
        let size = CHUNK_SIZE;
        local.x as usize + size * (local.y as usize + size * local.z as usize)
    }

    /// 读一个体素。
    pub fn get(&self, local: UVec3) -> VoxelType {
        self.voxels[Self::index(local)]
    }

    /// 写一个体素，返回数据是否真的变了。
    ///
    /// 「没变就不算脏」是渲染层避免无谓重建的判据。
    pub fn set(&mut self, local: UVec3, voxel: VoxelType) -> bool {
        let index = Self::index(local);
        if self.voxels[index] == voxel {
            return false;
        }
        self.voxels[index] = voxel;
        true
    }
}

/// 区块在区块坐标系中的位置（1 单位 = 1 区块 = [`CHUNK_SIZE`] 体素）。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkPos(pub IVec3);

impl ChunkPos {
    /// 区块原点对应的世界体素坐标。
    pub fn origin(self) -> IVec3 {
        self.0 * CHUNK_SIZE as i32
    }

    /// 世界体素坐标 → 所在区块（向下取整，负坐标同样正确）。
    pub fn from_voxel(voxel: IVec3) -> Self {
        let size = CHUNK_SIZE as i32;
        Self(IVec3::new(
            voxel.x.div_euclid(size),
            voxel.y.div_euclid(size),
            voxel.z.div_euclid(size),
        ))
    }

    /// 世界体素坐标 → 区块内局部坐标。
    pub fn local(self, voxel: IVec3) -> UVec3 {
        (voxel - self.origin()).as_uvec3()
    }

    /// 区块原点对应的世界坐标（表现层网格的挂载点）。
    pub fn to_world_translation(self) -> Vec3 {
        self.origin().as_vec3()
    }
}

/// 挂在需要流式加载周围区块的实体上（玩家 / 相机 / 观察点）。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkLoader {
    /// 以所在区块为中心，向各轴扩展的区块数
    pub radius: IVec3,
}

impl Default for ChunkLoader {
    fn default() -> Self {
        // 默认多带上下各一层：地表层（y = -1）与地表之上都要有
        Self {
            radius: IVec3::new(0, 1, 0),
        }
    }
}

/// 已修改区块标记：不会被流式卸载（玩家改过的地形不该消失）。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ChunkPinned;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_reports_only_real_changes() {
        let mut chunk = Chunk::empty();
        let local = UVec3::new(1, 2, 3);
        assert!(!chunk.set(local, VoxelType::Air), "写入相同值不应报告变更");
        assert!(chunk.set(local, VoxelType::Stone));
        assert_eq!(chunk.get(local), VoxelType::Stone);
    }

    #[test]
    fn chunk_pos_maps_negative_voxels_downwards() {
        assert_eq!(
            ChunkPos::from_voxel(IVec3::new(0, -1, 0)),
            ChunkPos(IVec3::new(0, -1, 0)),
            "-1 应落在 y = -1 区块，而不是 y = 0"
        );
        assert_eq!(
            ChunkPos::from_voxel(IVec3::new(-1, -33, 32)),
            ChunkPos(IVec3::new(-1, -2, 1))
        );
    }

    #[test]
    fn chunk_pos_converts_voxel_to_local_coordinates() {
        let pos = ChunkPos(IVec3::new(0, -1, 0));
        assert_eq!(pos.origin(), IVec3::new(0, -32, 0));
        assert_eq!(pos.local(IVec3::new(3, -1, 5)), UVec3::new(3, 31, 5));
    }
}
