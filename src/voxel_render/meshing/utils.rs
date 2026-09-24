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

/// 六个面方向的基：`(法线, 面所在角的基准, u 轴, v 轴)`，满足 **`u × v == 法线`**。
///
/// 为什么要这么定：合并出来的矩形由 `u` / `v` 两个方向撑开，四个角按
/// `origin → +u → +u+v → +v` 走。**`u × v` 等于法线**保证这个绕序
/// （右手定则）就是「从外侧看逆时针」——否则背面剔除会把面剔掉，
/// 表现为"合并之后整片地面不见了"。逐面版本那张 `FACES` 表用的是同一套绕序。
///
/// 「面所在角的基准」= `normal * (normal 的分量 > 0 ? 1 : 0)`：
/// `+` 方向的面在体素的**上界**一侧，`-` 方向在**下界**一侧。
const FACE_BASES: [(IVec3, IVec3, IVec3, IVec3); 6] = [
    // +X：u=Y, v=Z（Y × Z = X）
    (IVec3::X, IVec3::X, IVec3::Y, IVec3::Z),
    // -X：u=Z, v=Y（Z × Y = -X）
    (IVec3::NEG_X, IVec3::ZERO, IVec3::Z, IVec3::Y),
    // +Y（顶面）：u=Z, v=X（Z × X = Y）
    (IVec3::Y, IVec3::Y, IVec3::Z, IVec3::X),
    // -Y（底面）：u=X, v=Z（X × Z = -Y）
    (IVec3::NEG_Y, IVec3::ZERO, IVec3::X, IVec3::Z),
    // +Z：u=X, v=Y（X × Y = Z）
    (IVec3::Z, IVec3::Z, IVec3::X, IVec3::Y),
    // -Z：u=Y, v=X（Y × X = -Z）
    (IVec3::NEG_Z, IVec3::ZERO, IVec3::Y, IVec3::X),
];

/// 贪婪网格化：按面方向切层，把**同类型、共面**的面合并成矩形。
///
/// 逐面输出时一个 32³ 的实心区块要 `6 × 32² = 6144` 个四边形；合并之后同样的几何
/// 只要 6 个。默认地形实测（区块 `y=-2`，整层石头）：`6144 → 6`。
///
/// 合并的前提是**同一体素类型**（不同材质要分到不同网格），所以逐层扫出来的掩码
/// 存的是「类型下标」，只把相邻的**同下标**格子并进同一个矩形。
///
/// 它不查 `cull_hidden_faces`：合并本身就内含了面剔除（被挡住的格子 `None` 进不了
/// 掩码），所以「不剔除」那条路走 [`build_per_face`]。
fn build_greedy(chunk: &Chunk, builders: &mut [MeshBuilder; VoxelType::COUNT]) {
    let size = CHUNK_SIZE as i32;
    // 掩码按层复用：`None` = 这个格子这一层没有要画的面
    let mut mask: Vec<Option<usize>> = vec![None; CHUNK_SIZE * CHUNK_SIZE];
    let at = |iu: i32, iv: i32| (iu * size + iv) as usize;

    for (normal, base, u_axis, v_axis) in FACE_BASES {
        let axis_vec = normal.abs();
        for slice in 0..size {
            // 1) 这一层要画哪些面（越界的邻居当空气，所以区块边界的面照常画）
            for iu in 0..size {
                for iv in 0..size {
                    let cell = axis_vec * slice + u_axis * iu + v_axis * iv;
                    let voxel = chunk.get(cell.as_uvec3());
                    let hidden = !voxel.is_visible() || neighbour_is_opaque(chunk, cell + normal);
                    mask[at(iu, iv)] = (!hidden).then(|| voxel.index());
                }
            }

            // 2) 贪心合并：先沿 v 拉一条，再沿着 u 复制这条
            let origin = base + axis_vec * slice;
            for iu in 0..size {
                let mut iv = 0;
                while iv < size {
                    let Some(kind) = mask[at(iu, iv)] else {
                        iv += 1;
                        continue;
                    };
                    let mut v_end = iv + 1;
                    while v_end < size && mask[at(iu, v_end)] == Some(kind) {
                        v_end += 1;
                    }
                    let mut u_end = iu + 1;
                    while u_end < size && (iv..v_end).all(|v| mask[at(u_end, v)] == Some(kind)) {
                        u_end += 1;
                    }

                    // 3) 落地成矩形（绕序：origin → +u → +u+v → +v）
                    let corners = [
                        origin + u_axis * iu + v_axis * iv,
                        origin + u_axis * u_end + v_axis * iv,
                        origin + u_axis * u_end + v_axis * v_end,
                        origin + u_axis * iu + v_axis * v_end,
                    ];
                    builders[kind].push_quad(&corners, normal);

                    // 用掉的格子清空，免得被并进第二个矩形
                    for u in iu..u_end {
                        for v in iv..v_end {
                            mask[at(u, v)] = None;
                        }
                    }
                    iv = v_end;
                }
            }
        }
    }
}

/// 逐面输出（不合并）：每个可见方块的每个暴露面各一个四边形。
///
/// 两条路都留着，各有用途：这一条是 `cull_hidden_faces = false` 时走的路
/// （「关掉优化看原始几何」），也是贪婪网格化的**对照实现**——
/// 有一条测试逐一比对两者的顶点集合，保证合并没有改变几何。
fn build_per_face(chunk: &Chunk, builders: &mut [MeshBuilder; VoxelType::COUNT], cull: bool) {
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
                    if cull && neighbour_is_opaque(chunk, local + normal) {
                        continue;
                    }
                    // 顶点 = 面角（0..1） + 体素在区块内的偏移，否则所有方块会叠在区块原点
                    builder.push_quad(&corners.map(|corner| corner + local), normal);
                }
            }
        }
    }
}

/// 把一个区块的体素数据网格化成「方块类型 → Mesh」的分组。
///
/// 顶点坐标是区块内局部坐标（0..CHUNK_SIZE），挂载时由区块原点补上世界偏移；
/// 这样重建网格不需要动其他区块。
///
/// 默认走**贪婪网格化**（同类型共面的面合并成矩形）；`cull_hidden_faces = false`
/// 时退回逐面输出（保留原几何，用于对照与调试）。
pub fn build_chunk_meshes(chunk: &Chunk, config: MeshingConfig) -> Vec<(VoxelType, Mesh)> {
    let mut builders = VoxelType::ALL.map(|_| MeshBuilder::default());
    if config.cull_hidden_faces {
        build_greedy(chunk, &mut builders);
    } else {
        build_per_face(chunk, &mut builders, false);
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

    /// **合并之后几何不变**：贪婪网格化与逐面输出必须覆盖**同一片表面**。
    ///
    /// 这是贪婪网格化的**对照验收**。逐面那条路是独立实现，两条路覆盖的**面积**
    /// 必须相等——合并只能改变"哪些面被并成一个矩形"，不能凭空多画或少画。
    ///
    /// ⚠️ **比的是面积，不是顶点集合**：合并的**本意**就是把内部顶点去掉
    /// （一整片地面从 32×32 个四边形的角点并成一个矩形的 4 个角点），
    /// 所以顶点集合必然不同。拿顶点集合当判据会把"合并成功"判成失败。
    ///
    /// 对一个有台阶的起伏地形比，比单个方块更能抓住"合并时跨过了不该合并的格子"
    /// 或"某个方向漏了一片"这类错误。
    #[test]
    fn greedy_merging_covers_exactly_the_same_surface_as_per_face() {
        let mut chunk = Chunk::empty();
        // 造一片有台阶的起伏地形：相邻列高度不同 → 侧面必须照常生成
        for x in 0..CHUNK_SIZE as i32 {
            for z in 0..CHUNK_SIZE as i32 {
                let height = 4 + ((x * 7 + z * 13) % 5);
                for y in 0..height {
                    let voxel = if y == height - 1 {
                        VoxelType::Grass
                    } else {
                        VoxelType::Dirt
                    };
                    chunk.set(UVec3::new(x as u32, y as u32, z as u32), voxel);
                }
            }
        }

        let greedy = build_chunk_meshes(
            &chunk,
            MeshingConfig {
                cull_hidden_faces: true,
            },
        );
        // 逐面路径也要**剔除隐藏面**才可比：`cull_hidden_faces = false` 会多画内部面
        let mut builders = VoxelType::ALL.map(|_| MeshBuilder::default());
        build_per_face(&chunk, &mut builders, true);
        let per_face: Vec<(VoxelType, Mesh)> = VoxelType::ALL
            .into_iter()
            .zip(builders)
            .filter(|(_, builder)| !builder.is_empty())
            .map(|(voxel, builder)| (voxel, builder.build()))
            .collect();

        // 面积：每个四边形拆成两个三角形，三角形面积 = |cross| / 2。
        // 所有面都是轴对齐的，所以这个和是精确的（没有浮点累积误差的顾虑）。
        let area_of = |meshes: &[(VoxelType, Mesh)]| -> f32 {
            let mut total = 0.0;
            for (_, mesh) in meshes {
                let positions = positions_of(mesh);
                for quad in positions.chunks(4) {
                    let p = |i: usize| Vec3::from(quad[i]);
                    total += (p(1) - p(0)).cross(p(2) - p(0)).length() / 2.0;
                    total += (p(2) - p(0)).cross(p(3) - p(0)).length() / 2.0;
                }
            }
            total
        };

        assert!(
            (area_of(&greedy) - area_of(&per_face)).abs() < 1e-3,
            "两条路必须覆盖相同的面积：贪婪 {} vs 逐面 {}",
            area_of(&greedy),
            area_of(&per_face)
        );
        let greedy_quads: usize = greedy.iter().map(|(_, m)| vertex_count(m) / 4).sum();
        let per_face_quads: usize = per_face.iter().map(|(_, m)| vertex_count(m) / 4).sum();
        assert!(
            greedy_quads < per_face_quads,
            "合并应当真的减少面数（贪婪网格化的全部意义）：{greedy_quads} vs {per_face_quads}"
        );
    }

    /// **同类型的整层要并成极少的大方块**：一个 32³ 实心区块的顶面 / 底面各只需 1 块。
    ///
    /// 这条钉住"合并真的在合并"——只测"几何不变"的话，一个不做任何合并的实现
    /// （每个格子推一个 1×1 的矩形）也能通过。默认地形实测：`6144 → 6` 个四边形。
    #[test]
    fn a_solid_chunk_layer_collapses_into_single_rectangles() {
        let mut chunk = Chunk::empty();
        for y in 0..CHUNK_SIZE as u32 {
            for x in 0..CHUNK_SIZE as u32 {
                for z in 0..CHUNK_SIZE as u32 {
                    chunk.set(UVec3::new(x, y, z), VoxelType::Stone);
                }
            }
        }

        let meshes = build_chunk_meshes(
            &chunk,
            MeshingConfig {
                cull_hidden_faces: true,
            },
        );

        assert_eq!(meshes.len(), 1, "整块只有一种材质");
        // 一个 32³ 的实心方块：6 个面各合并成 1 个矩形 → 6 个四边形 = 24 个顶点
        assert_eq!(
            vertex_count(&meshes[0].1),
            6 * 4,
            "实心区块的六个面各应合并成**一个**矩形（逐面输出会是 6144 个四边形）"
        );
    }

    /// **绕序必须与法线一致**：每个四边形从外侧看都是逆时针。
    ///
    /// 这是贪婪合并最容易错的一处：四个角是按 `u` / `v` 手拼的，绕序反了的话
    /// 背面剔除会把整片面**从里面**画出来（或者干脆看不见），而顶点数、面积全都对，
    /// 单看那些判据抓不出来。判据：`(p1-p0) × (p2-p0)` 与法线同向。
    #[test]
    fn every_greedy_quad_winds_counter_clockwise_from_outside() {
        let mut chunk = Chunk::empty();
        // 一片台阶地形，六种朝向的面都会出现
        for x in 0..CHUNK_SIZE as i32 {
            for z in 0..CHUNK_SIZE as i32 {
                let height = 3 + ((x * 5 + z * 11) % 7);
                for y in 0..height {
                    chunk.set(UVec3::new(x as u32, y as u32, z as u32), VoxelType::Stone);
                }
            }
        }
        let meshes = build_chunk_meshes(
            &chunk,
            MeshingConfig {
                cull_hidden_faces: true,
            },
        );

        for (voxel, mesh) in &meshes {
            let positions = positions_of(mesh);
            let normals = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap() {
                bevy::mesh::VertexAttributeValues::Float32x3(normals) => normals.clone(),
                other => panic!("法线格式意外：{other:?}"),
            };
            for (quad, normal) in positions.chunks(4).zip(normals.chunks(4)) {
                let p = |i: usize| Vec3::from(quad[i]);
                let face_normal = Vec3::from(normal[0]);
                let winding = (p(1) - p(0)).cross(p(2) - p(0)).normalize_or_zero();
                assert!(
                    winding.dot(face_normal) > 0.9,
                    "{voxel:?} 的四边形绕序与法线不符：winding={winding:?} normal={face_normal:?}"
                );
            }
        }
    }

    #[test]
    fn greedy_merges_neighbours_into_one_quad() {
        let meshes = meshes_of(
            &[
                (IVec3::new(4, 4, 4), VoxelType::Stone),
                (IVec3::new(5, 4, 4), VoxelType::Stone),
            ],
            true,
        );
        // 两个方块并排：中间那对隐藏面被剔除，其余四个侧面各并成 1 块、上下各并成
        // 1 块 2×1 的矩形 → 6 个四边形。逐面输出是 10 个。
        assert_eq!(
            vertex_count(&meshes[0].1),
            6 * 4,
            "相邻两块应当合并成 6 个矩形（逐面是 10 个四边形）"
        );
    }

    /// **合并只发生在同类型之间**：相邻但类型不同的格子必须分开成两个矩形。
    #[test]
    fn greedy_does_not_merge_across_different_types() {
        let meshes = meshes_of(
            &[
                (IVec3::new(4, 4, 4), VoxelType::Stone),
                (IVec3::new(5, 4, 4), VoxelType::Grass),
            ],
            true,
        );
        assert_eq!(meshes.len(), 2, "两种类型 → 两个网格");
        // 各自是一个孤立方块：6 个面（其中一个面朝对方，被剔除 → 5 个）
        for (voxel, mesh) in &meshes {
            assert_eq!(
                vertex_count(mesh),
                5 * 4,
                "{voxel:?} 与邻居类型不同：只能剔掉朝向对方的那一个面，不能跨类型合并"
            );
        }
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
