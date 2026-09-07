use bevy::prelude::*;

#[derive(SceneComponent, Default, Clone)]
#[scene(GroundGridParams)]
pub struct GroundGrid;

#[derive(Debug, Clone)]
pub struct GroundGridParams {
    // x, z 方向的尺寸
    pub width: f32,
    pub height: f32,
    // 每个网格单元的边长（默认 1.0）
    pub cell_size: f32,
    pub color: Color,
}

impl Default for GroundGridParams {
    fn default() -> Self {
        Self {
            width: 30.0,
            height: 30.0,
            cell_size: 1.0, // 默认格子边长 1.0
            color: Color::WHITE,
        }
    }
}

impl GroundGrid {
    fn scene(params: GroundGridParams) -> impl Scene {
        let half_w = params.width / 2.0;
        let half_h = params.height / 2.0;
        let step = params.cell_size; // 格子边长

        // 存储所有线段的端点（每两个 Vec3 构成一条线段）
        let mut vertices = Vec::new();

        // 1. 平行于 X 轴的线条（沿 Z 方向移动）
        // 从 -half_h 开始，以 step 为步长，直到 >= half_h（包含边界）
        let mut z = -half_h;
        while z <= half_h + 1e-6 {
            vertices.push(Vec3::new(-half_w, 0.0, z));
            vertices.push(Vec3::new(half_w, 0.0, z));
            z += step;
        }

        // 2. 平行于 Z 轴的线条（沿 X 方向移动）
        let mut x = -half_w;
        while x <= half_w + 1e-6 {
            vertices.push(Vec3::new(x, 0.0, -half_h));
            vertices.push(Vec3::new(x, 0.0, half_h));
            x += step;
        }

        // 构建 Mesh（LineList）
        let mut mesh = Mesh::new(bevy::mesh::PrimitiveTopology::LineList, default());
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vertices.iter().map(|v| [v.x, v.y, v.z]).collect::<Vec<_>>(),
        );

        bsn! {
            #GroundGrid
            Mesh3d(asset_value(mesh))
            MeshMaterial3d<StandardMaterial>(asset_value(StandardMaterial {
                base_color: params.color,
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..Default::default()
            }))
            Transform::from_xyz(params.width / 2.0, 0.0, params.height / 2.0)
        }
    }
}

#[derive(SceneComponent, Default, Clone)]
#[scene(GroundParams)]
pub struct Ground;

#[derive(Debug, Clone)]
pub struct GroundParams {
    // x, z
    pub width: f32,
    pub height: f32,
    pub color: Color,
}

impl Default for GroundParams {
    fn default() -> Self {
        Self {
            width: 30.0,
            height: 30.0,
            color: Color::srgb(0.08, 0.10, 0.12),
        }
    }
}

impl Ground {
    fn scene(params: GroundParams) -> impl Scene {
        let width = params.width;
        let height = params.height;
        bsn! {
            #Ground
            Mesh3d(asset_value(Plane3d::default()
                .mesh()
                .size(width, height)))
            MeshMaterial3d<StandardMaterial>(asset_value( StandardMaterial {
                base_color: params.color,
                ..default()
            }))
            Transform::from_xyz(params.width / 2.0, -0.01, params.height / 2.0)
        }
    }
}
