//! 体素类型：世界数据的最小单位。
//!
//! 纯数据枚举，不带任何引擎类型——渲染层（`voxel_render`）只按它查材质。

/// 方块类型。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VoxelType {
    /// 空气：不渲染、不遮挡、不阻挡
    #[default]
    Air,
    Grass,
    Dirt,
    Stone,
    /// 水体：可渲染但不遮挡（水面下的地形仍要画出来）
    Water,
    Wood,
    Leaves,
}

impl VoxelType {
    /// 类型数量。
    pub const COUNT: usize = 7;

    /// 全部类型（渲染层遍历注册材质用）。
    pub const ALL: [VoxelType; Self::COUNT] = [
        VoxelType::Air,
        VoxelType::Grass,
        VoxelType::Dirt,
        VoxelType::Stone,
        VoxelType::Water,
        VoxelType::Wood,
        VoxelType::Leaves,
    ];

    /// 类型下标（`0..COUNT`，供定长表按类型索引）。
    pub fn index(self) -> usize {
        match self {
            Self::Air => 0,
            Self::Grass => 1,
            Self::Dirt => 2,
            Self::Stone => 3,
            Self::Water => 4,
            Self::Wood => 5,
            Self::Leaves => 6,
        }
    }

    /// 是否遮挡相邻面（网格化时剔除被它挡住的邻居面）。
    pub fn is_opaque(self) -> bool {
        !matches!(self, Self::Air | Self::Water)
    }

    /// 是否需要生成几何（空气不需要）。
    pub fn is_visible(self) -> bool {
        !matches!(self, Self::Air)
    }

    /// 调试 / 日志用名称（标识符保持英文）。
    pub fn name(self) -> &'static str {
        match self {
            Self::Air => "air",
            Self::Grass => "grass",
            Self::Dirt => "dirt",
            Self::Stone => "stone",
            Self::Water => "water",
            Self::Wood => "wood",
            Self::Leaves => "leaves",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn air_and_water_do_not_occlude_neighbours() {
        assert!(!VoxelType::Air.is_opaque(), "空气不应遮挡邻居面");
        assert!(!VoxelType::Water.is_opaque(), "水体不遮挡：水下地形仍要画");
        assert!(VoxelType::Grass.is_opaque());
        assert!(VoxelType::Stone.is_opaque());
    }

    #[test]
    fn only_air_is_invisible() {
        assert!(!VoxelType::Air.is_visible());
        for voxel in VoxelType::ALL.into_iter().filter(|v| *v != VoxelType::Air) {
            assert!(voxel.is_visible(), "{} 应当可见", voxel.name());
        }
    }

    #[test]
    fn index_is_a_dense_unique_key() {
        let mut seen = [false; VoxelType::COUNT];
        for voxel in VoxelType::ALL {
            assert!(
                !seen[voxel.index()],
                "{} 的下标与其它类型冲突",
                voxel.name()
            );
            seen[voxel.index()] = true;
        }
        assert!(seen.into_iter().all(|used| used), "下标应当覆盖 0..COUNT");
    }
}
