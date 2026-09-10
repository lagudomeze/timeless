//! 网格生成工具：区块体素数据 → 按材质分组的 [`Mesh`]。
//!
//! 纯计算（不碰 ECS），因此可以丢进异步任务池，也可以直接单测。

use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::voxel_render::lighting::face_shade;
use crate::world::VoxelType;
use crate::world::chunk::{CHUNK_SIZE, Chunk};

use super::resources::MeshingConfig;

/// 立方体六个面：法线 + 四角（体素局部坐标，逆时针朝外）。
const FACES: [(IVec3, [IVec3; 4]); 6] = [
    // +X
    (
        IVec3::new(1, 0, 0),
        [
            IVec3::new(1, 0, 0),
            IVec3::new(1, 1, 0),
            IVec3::new(1, 1, 1),
            IVec3::new(1, 0, 1),
        ],
    ),
    // -X
    (
        IVec3::new(-1, 0, 0),
        [
            IVec3::new(0, 0, 0),
            IVec3::new(0, 0, 1),
            IVec3::new(0, 1, 1),
            IVec3::new(0, 1, 0),
        ],
    ),
    // +Y（顶面）
    (
        IVec3::new(0, 1, 0),
        [
            IVec3::new(0, 1, 0),
            IVec3::new(0, 1, 1),
            IVec3::new(1, 1, 1),
            IVec3::new(1, 1, 0),
        ],
    ),
    // -Y（底面）
    (
        IVec3::new(0, -1, 0),
        [
            IVec3::new(0, 0, 0),
            IVec3::new(1, 0, 0),
            IVec3::new(1, 0, 1),
            IVec3::new(0, 0, 1),
        ],
    ),
    // +Z
    (
        IVec3::new(0, 0, 1),
        [
            IVec3::new(0, 0, 1),
            IVec3::new(1, 0, 1),
            IVec3::new(1, 1, 1),
            IVec3::new(0, 1, 1),
        ],
    ),
    // -Z
    (
        IVec3::new(0, 0, -1),
        [
            IVec3::new(0, 0, 0),
            IVec3::new(0, 1, 0),
            IVec3::new(1, 1, 0),
            IVec3::new(1, 0, 0),
        ],
    ),
];

/// 单个方块类型的网格装配器（四个顶点属性 + 索引）。
#[derive(Default)]
struct MeshBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl MeshBuilder {
    /// 追加一个四边形（两个三角形），顶点色承载面朝向明暗。
    fn push_quad(&mut self, corners: &[IVec3; 4], normal: IVec3) {
        let base = self.positions.len() as u32;
        let shade = face_shade(normal);
        for corner in corners {
            self.positions.push(corner.as_vec3().to_array());
            self.normals.push(normal.as_vec3().to_array());
            self.colors.push([shade, shade, shade, 1.0]);
        }
        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    fn build(self) -> Mesh {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}

/// 区块是否是空气邻居（越界当作空气：区块边界的面照常画出来）。
fn neighbour_is_opaque(chunk: &Chunk, local: IVec3) -> bool {
    let size = CHUNK_SIZE as i32;
    if local.x < 0
        || local.y < 0
        || local.z < 0
        || local.x >= size
        || local.y >= size
        || local.z >= size
    {
        return false;
    }
    chunk.get(local.as_uvec3()).is_opaque()
}

/// 把一个区块的体素数据网格化成「方块类型 → Mesh」的分组。
///
/// 顶点坐标是区块内局部坐标（0..CHUNK_SIZE），挂载时由区块原点补上世界偏移；
/// 这样重建网格不需要动其他区块。
pub fn build_chunk_meshes(chunk: &Chunk, config: MeshingConfig) -> Vec<(VoxelType, Mesh)> {
    // 单趟扫描区块：每个方块把暴露的面推进「自己类型」的装配器
    let mut builders = VoxelType::ALL.map(|_| MeshBuilder::default());
    for z in 0..CHUNK_SIZE as i32 {
        for y in 0..CHUNK_SIZE as i32 {
            for x in 0..CHUNK_SIZE as i32 {
                let local = IVec3::new(x, y, z);
                let voxel = chunk.get(local.as_uvec3());
                if !voxel.is_visible() {
                    continue;
                }
                let builder = &mut builders[voxel.index()];
                for (normal, corners) in FACES {
                    if config.cull_hidden_faces && neighbour_is_opaque(chunk, local + normal) {
                        continue;
                    }
                    // 顶点 = 面角（0..1） + 体素在区块内的偏移，否则所有方块会叠在区块原点
                    builder.push_quad(&corners.map(|corner| corner + local), normal);
                }
            }
        }
    }

    // 按 VoxelType::ALL 的固定顺序输出，结果稳定（便于测试与复现）
    VoxelType::ALL
        .into_iter()
        .zip(builders)
        .filter(|(_, builder)| !builder.is_empty())
        .map(|(voxel, builder)| (voxel, builder.build()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertex_count(mesh: &Mesh) -> usize {
        mesh.count_vertices()
    }

    fn positions_of(mesh: &Mesh) -> Vec<[f32; 3]> {
        match mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .expect("网格应带顶点位置")
        {
            bevy::mesh::VertexAttributeValues::Float32x3(positions) => positions.clone(),
            other => panic!("顶点位置格式意外：{other:?}"),
        }
    }

    fn index_count(mesh: &Mesh) -> usize {
        match mesh.indices().expect("网格应带索引") {
            Indices::U16(indices) => indices.len(),
            Indices::U32(indices) => indices.len(),
        }
    }

    fn meshes_of(voxels: &[(IVec3, VoxelType)], cull: bool) -> Vec<(VoxelType, Mesh)> {
        let mut chunk = Chunk::empty();
        for (local, voxel) in voxels {
            chunk.set(local.as_uvec3(), *voxel);
        }
        build_chunk_meshes(
            &chunk,
            MeshingConfig {
                cull_hidden_faces: cull,
            },
        )
    }

    #[test]
    fn lone_voxel_emits_all_six_faces() {
        let meshes = meshes_of(&[(IVec3::new(1, 1, 1), VoxelType::Stone)], true);
        assert_eq!(meshes.len(), 1, "只有一种方块时只产出一个网格");
        assert_eq!(vertex_count(&meshes[0].1), 6 * 4);
        assert_eq!(index_count(&meshes[0].1), 6 * 6);
    }

    #[test]
    fn hidden_faces_between_neighbours_are_culled() {
        let meshes = meshes_of(
            &[
                (IVec3::new(4, 4, 4), VoxelType::Stone),
                (IVec3::new(5, 4, 4), VoxelType::Stone),
            ],
            true,
        );
        assert_eq!(
            vertex_count(&meshes[0].1),
            10 * 4,
            "两块相邻方块共 12 个面，中间的 2 个面应被剔除"
        );
    }

    #[test]
    fn culling_can_be_disabled() {
        let meshes = meshes_of(
            &[
                (IVec3::new(4, 4, 4), VoxelType::Stone),
                (IVec3::new(5, 4, 4), VoxelType::Stone),
            ],
            false,
        );
        assert_eq!(vertex_count(&meshes[0].1), 12 * 4);
    }

    #[test]
    fn different_voxel_types_become_separate_meshes() {
        let meshes = meshes_of(
            &[
                (IVec3::new(0, 0, 0), VoxelType::Grass),
                (IVec3::new(2, 0, 0), VoxelType::Stone),
            ],
            true,
        );
        assert_eq!(meshes.len(), 2, "不同方块类型要分成不同网格（各自材质）");
        let kinds: Vec<VoxelType> = meshes.iter().map(|(voxel, _)| *voxel).collect();
        assert_eq!(kinds, vec![VoxelType::Grass, VoxelType::Stone]);
    }

    #[test]
    fn chunk_boundary_faces_are_kept() {
        let meshes = meshes_of(&[(IVec3::ZERO, VoxelType::Dirt)], true);
        assert_eq!(
            vertex_count(&meshes[0].1),
            6 * 4,
            "区块边缘的面没有邻居遮挡，必须保留"
        );
    }

    #[test]
    fn quads_are_offset_to_the_voxel_position() {
        let (x, y, z) = (5, 6, 7);
        let meshes = meshes_of(&[(IVec3::new(x, y, z), VoxelType::Stone)], true);
        let min = positions_of(&meshes[0].1)
            .into_iter()
            .fold([f32::MAX; 3], |min, position| {
                [
                    min[0].min(position[0]),
                    min[1].min(position[1]),
                    min[2].min(position[2]),
                ]
            });
        let max = positions_of(&meshes[0].1)
            .into_iter()
            .fold([f32::MIN; 3], |max, position| {
                [
                    max[0].max(position[0]),
                    max[1].max(position[1]),
                    max[2].max(position[2]),
                ]
            });
        assert_eq!(
            (min, max),
            (
                [x as f32, y as f32, z as f32],
                [x as f32 + 1.0, y as f32 + 1.0, z as f32 + 1.0]
            ),
            "方块的面必须落在它自己的格子上，而不是区块原点"
        );
    }

    #[test]
    fn a_far_corner_voxel_stays_at_the_far_corner() {
        let last = CHUNK_SIZE as i32 - 1;
        let meshes = meshes_of(&[(IVec3::splat(last), VoxelType::Stone)], true);
        let max = positions_of(&meshes[0].1)
            .into_iter()
            .fold([f32::MIN; 3], |max, position| {
                [
                    max[0].max(position[0]),
                    max[1].max(position[1]),
                    max[2].max(position[2]),
                ]
            });
        assert_eq!(
            max, [CHUNK_SIZE as f32; 3],
            "区块另一角的方块应当仍在另一角：整块区块不能被压到原点"
        );
    }
}
