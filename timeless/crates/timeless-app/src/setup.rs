//! # 应用层：伪 3D 场景搭建（3D 场景 + 2D 纸片）
//!
//! 编排职责：相机 / 光照 → 地面与装饰（见 `display::map`）→ 纸片单位资源与出生 →
//! 移动箭头资源（见 `display::hints`）→ HUD（见 `display::hud`）。
//! 规则逻辑全部在领域层与应用层领域模块（combat / movement / timeline / menu），
//! 本模块只做组合，不写规则。

use bevy::prelude::*;

use crate::combat::{AttackFrame, AttackRange, Damage, Enemy, Health, Impact, Player, Stamina};
use crate::display::camera::MainCamera;
use crate::display::hover::spawn_hover_info;
use crate::display::hud::{spawn_battle_log, spawn_hud};
use crate::display::map::{
    ENEMY_SPAWN, GRID_SIZE, PLAYER_SPAWN, cell_x, cell_z, spawn_decorations, spawn_ground,
};
use crate::display::unit::{
    BILLBOARD_HEIGHT, BILLBOARD_WIDTH, Billboard, GroundShadow, PaperAssets, SHADOW_RADIUS,
    UnitRoot,
};
use crate::movement::{FireballAssets, Position};

/// 场景搭建：相机、光照、地面、装饰、纸片单位、HUD
pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // 相机：斜 45° 俯视，看向地图中心
    commands.spawn((
        MainCamera,
        Camera3d::default(),
        Transform::from_xyz(0.0, GRID_SIZE as f32 * 0.85, GRID_SIZE as f32 * 0.85)
            .looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // 光照：太阳（带阴影）+ 环境光
    commands.spawn((
        DirectionalLight {
            illuminance: 10_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform {
            translation: Vec3::new(0.0, 18.0, 0.0),
            rotation: Quat::from_rotation_x(-std::f32::consts::FRAC_PI_4)
                * Quat::from_rotation_y(std::f32::consts::FRAC_PI_4),
            ..default()
        },
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 400.0,
        ..default()
    });

    // 地面与装饰（Kenney CC0：Prototype Textures + Nature Kit）
    spawn_ground(&mut commands, &mut meshes, &mut materials, &asset_server);
    spawn_decorations(&mut commands, &asset_server);

    // 纸片单位共享资源
    let paper = PaperAssets {
        billboard: meshes.add(Rectangle::new(BILLBOARD_WIDTH, BILLBOARD_HEIGHT)),
        shadow: meshes.add(Circle::new(SHADOW_RADIUS)),
        player: materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.45, 1.0),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        }),
        enemy: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.25, 0.2),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        }),
        shadow_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(0.0, 0.0, 0.0, 0.4),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        }),
    };
    commands.insert_resource(paper.clone());
    spawn_combatants(&mut commands, &paper);

    // 火球投射物共享资源（施放时由 combat::resolve_system 复用）
    commands.insert_resource(FireballAssets {
        mesh: meshes.add(Sphere::new(0.18)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.45, 0.1),
            unlit: true,
            ..default()
        }),
    });

    // HUD：屏幕底部状态文本（单位头顶标签在 3D 场景中暂用 HUD 展示）
    spawn_hud(&mut commands, &asset_server);
    // 战斗日志：屏幕右下角
    spawn_battle_log(&mut commands, &asset_server);
    // 悬停坐标读数：屏幕右上角专用区域
    spawn_hover_info(&mut commands, &asset_server);
}

/// 生成玩家与敌人：纸片单位（Billboard）+ 贴地阴影（重置时复用）
pub fn spawn_combatants(commands: &mut Commands, paper: &PaperAssets) {
    // 玩家：蓝纸片，帧 4 / 射程 1 / 破势 3 / 伤害 10，HP 20
    spawn_combatant(
        commands,
        paper,
        PLAYER_SPAWN,
        Player,
        paper.player.clone(),
        20,
        4,
        1,
        3,
        10,
    );
    // 敌人：红纸片，帧 5 / 射程 1 / 破势 2 / 伤害 8，HP 16
    spawn_combatant(
        commands,
        paper,
        ENEMY_SPAWN,
        Enemy,
        paper.enemy.clone(),
        16,
        5,
        1,
        2,
        8,
    );
}

/// 单个战斗单位：根节点（承载逻辑坐标与意图/属性组件）+ 纸片 / 阴影两个子实体。
/// 攻击属性拆成 `AttackFrame` / `AttackRange` / `Impact` / `Damage` 四个小组件，
/// 由 `combat::resolve_system` 在裁决前组装成领域层 `AttackStats`。
#[allow(clippy::too_many_arguments)]
fn spawn_combatant<M: Component>(
    commands: &mut Commands,
    paper: &PaperAssets,
    cell: (i32, i32),
    marker: M,
    material: Handle<StandardMaterial>,
    max_hp: u32,
    frame: u32,
    range: u32,
    impact: u32,
    damage: u32,
) {
    let x = cell_x(cell.0);
    let z = cell_z(cell.1);

    commands
        .spawn((
            marker,
            UnitRoot,
            Position::new(cell.0, cell.1),
            Health::new(max_hp),
            Stamina::new(5),
            AttackFrame(frame),
            AttackRange(range),
            Impact(impact),
            Damage(damage),
            Transform::from_xyz(x, 0.0, z),
            // 必须显式携带 Visibility：子实体的 InheritedVisibility 需要父级同步
            Visibility::default(),
        ))
        .with_children(|parent| {
            // 纸片：相对根节点抬高；billboard 只转纸片自身，不影响阴影
            parent.spawn((
                Billboard,
                Mesh3d(paper.billboard.clone()),
                MeshMaterial3d(material),
                Transform::from_xyz(0.0, BILLBOARD_HEIGHT / 2.0, 0.0),
            ));
            // 贴地阴影：相对根节点贴地；根节点不旋转，阴影保持水平
            parent.spawn((
                GroundShadow,
                Mesh3d(paper.shadow.clone()),
                MeshMaterial3d(paper.shadow_mat.clone()),
                Transform::from_xyz(0.0, 0.02, 0.0)
                    .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            ));
        });
}
