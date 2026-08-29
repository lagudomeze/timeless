//! # 地图表现：网格几何常量、地面铺设与装饰摆放
//!
//! - 地面：深色大底板 + 每格草地贴图（Kenney Prototype Textures，CC0，512px 平铺）
//! - 装饰：Kenney Nature Kit 低模 GLB（树/石/灌木/花/草），自包含、无外部贴图
//! - 出生点与网格几何常量也在此处（移动/菜单等逻辑模块引用时保持单一出处）

use std::collections::HashSet;

use bevy::prelude::*;

use timeless_domain::grid::GridPos;

/// 地图网格尺寸（21×21，为战棋推演留足空间）
pub const GRID_SIZE: i32 = 21;
/// 单格世界尺寸（1 个单位，与 Kenney 模型比例一致）
pub const CELL_SIZE: f32 = 1.0;

/// 玩家 / 敌人出生点（伪 3D 场景第一版：左右对称，距离 4 格）
pub const PLAYER_SPAWN: (i32, i32) = (8, 10);
pub const ENEMY_SPAWN: (i32, i32) = (12, 10);

/// 装饰模型表：(资产路径, 最小缩放, 最大缩放)
const DECOR_MODELS: &[(&str, f32, f32)] = &[
    ("models/nature/tree_default.glb", 1.2, 1.7),
    ("models/nature/tree_default_dark.glb", 1.2, 1.7),
    ("models/nature/tree_oak.glb", 1.3, 1.8),
    ("models/nature/tree_small.glb", 0.9, 1.3),
    ("models/nature/tree_pineRoundA.glb", 1.0, 1.5),
    ("models/nature/tree_pineTallA.glb", 1.1, 1.5),
    ("models/nature/tree_palm.glb", 1.1, 1.5),
    ("models/nature/rock_largeA.glb", 0.5, 0.9),
    ("models/nature/rock_largeB.glb", 0.5, 0.9),
    ("models/nature/rock_smallA.glb", 0.4, 0.7),
    ("models/nature/stone_smallA.glb", 0.4, 0.7),
    ("models/nature/plant_bush.glb", 0.7, 1.1),
    ("models/nature/plant_bushSmall.glb", 0.5, 0.8),
    ("models/nature/flower_redA.glb", 0.6, 0.9),
    ("models/nature/flower_yellowA.glb", 0.6, 0.9),
    ("models/nature/grass.glb", 0.7, 1.0),
    ("models/nature/log.glb", 0.8, 1.1),
    ("models/nature/stump_round.glb", 0.8, 1.1),
];

/// 网格坐标 → 世界坐标（XZ 平面，地图居中；Y 为高度轴）
pub fn cell_x(x: i32) -> f32 {
    (x as f32 - (GRID_SIZE as f32 - 1.0) / 2.0) * CELL_SIZE
}
pub fn cell_z(y: i32) -> f32 {
    (y as f32 - (GRID_SIZE as f32 - 1.0) / 2.0) * CELL_SIZE
}

/// 网格坐标（可带小数，投射物插值用）→ 世界坐标
pub fn cell_x_f(x: f32) -> f32 {
    (x - (GRID_SIZE as f32 - 1.0) / 2.0) * CELL_SIZE
}
pub fn cell_z_f(y: f32) -> f32 {
    (y - (GRID_SIZE as f32 - 1.0) / 2.0) * CELL_SIZE
}

/// 世界坐标（地面 y≈0 平面）→ 网格坐标；超出地图范围返回 `None`
pub fn world_to_cell(world: Vec3) -> Option<GridPos> {
    let x = (world.x / CELL_SIZE + (GRID_SIZE as f32 - 1.0) / 2.0).round() as i32;
    let y = (world.z / CELL_SIZE + (GRID_SIZE as f32 - 1.0) / 2.0).round() as i32;
    let in_bounds = (0..GRID_SIZE).contains(&x) && (0..GRID_SIZE).contains(&y);
    in_bounds.then_some(GridPos::new(x, y))
}

/// 地面：深色大底板 + 每格一块草地贴图（共享 mesh/material；尺寸留 2% 间隙形成网格线）
pub fn spawn_ground(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
) {
    // 深色大底板（tile 之间的“网格线”底色）
    commands.spawn((
        Mesh3d(
            meshes.add(
                Plane3d::default()
                    .mesh()
                    .size(GRID_SIZE as f32 * CELL_SIZE, GRID_SIZE as f32 * CELL_SIZE),
            ),
        ),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.10, 0.12),
            ..default()
        })),
        Transform::from_xyz(0.0, -0.01, 0.0),
    ));

    let grass = asset_server.load::<Image>("textures/ground/grass.png");
    let grass_material = materials.add(StandardMaterial {
        base_color_texture: Some(grass),
        ..default()
    });
    let tile_mesh = meshes.add(Plane3d::default().mesh().size(CELL_SIZE, CELL_SIZE));
    let mut tiles = 0;
    for x in 0..GRID_SIZE {
        for y in 0..GRID_SIZE {
            commands.spawn((
                Mesh3d(tile_mesh.clone()),
                MeshMaterial3d(grass_material.clone()),
                Transform::from_xyz(cell_x(x), 0.0, cell_z(y)).with_scale(Vec3::splat(0.98)),
            ));
            tiles += 1;
        }
    }
    info!("[场景] 地面铺设完成：{GRID_SIZE}×{GRID_SIZE} 共 {tiles} 格（Kenney CC0 草地）");
}

/// 装饰：Kenney Nature Kit 低模 GLB（自包含，无外部贴图）。
/// 固定种子 LCG 保证开局布局一致；出生点周围留空避免遮挡单位。
pub fn spawn_decorations(commands: &mut Commands, asset_server: &AssetServer) {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut rng = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) & 0x7FFF_FFFF) as f32 / 0x7FFF_FFFF as f32
    };

    let target = (GRID_SIZE * GRID_SIZE) as usize / 6;
    let mut occupied = HashSet::new();
    let mut placed = 0;
    let mut guard = 0;
    while placed < target && guard < target * 20 {
        guard += 1;
        let x = (rng() * GRID_SIZE as f32) as i32;
        let y = (rng() * GRID_SIZE as f32) as i32;
        // 出生点周围两格保持空旷
        if (x - PLAYER_SPAWN.0).abs() <= 1 && (y - PLAYER_SPAWN.1).abs() <= 1 {
            continue;
        }
        if (x - ENEMY_SPAWN.0).abs() <= 1 && (y - ENEMY_SPAWN.1).abs() <= 1 {
            continue;
        }
        if !occupied.insert((x, y)) {
            continue;
        }

        let (path, s_min, s_max) = DECOR_MODELS[(rng() * DECOR_MODELS.len() as f32) as usize];
        let scale = s_min + (s_max - s_min) * rng();
        let scene: Handle<WorldAsset> =
            asset_server.load(GltfAssetLabel::Scene(0).from_asset(path));
        commands.spawn((
            WorldAssetRoot(scene),
            Transform::from_xyz(cell_x(x), 0.0, cell_z(y)).with_scale(Vec3::splat(scale)),
        ));
        placed += 1;
    }
    info!("[场景] 装饰放置完成：{placed} 个（Kenney Nature Kit，CC0）");
}
