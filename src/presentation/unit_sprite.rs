//! 单位外观：面向相机的 2D 精灵 + 贴地的黑色阴影。
//!
//! 伪 3D 方案（见 [docs/assets.md](../../../docs/assets.md)）：树木 / 石头继续用
//! 3D 模型，玩家 / 敌人换成 2D 纸片。两个约定决定了这里的写法：
//!
//! - **纸片绕 Y 轴对准相机**：单位自己不转身（移动只改位置），精灵每帧只调偏航角、
//!   俯仰保持竖直。立起来的纸片配贴地阴影才有「站在地上」的实感；
//! - **高度用贴地阴影表示**：阴影永远画在单位**正下方的地表**上（高度取自
//!   [`crate::world`] 的地表函数），单位离地越高，阴影越小、离脚越远——
//!   脚底与阴影之间的距离就是可以直接读出来的高度差。
//!
//! 精灵与阴影都是单位实体的**子节点**（零件在本域，组装在 [`crate::spawn`]）。
//! 因此单位的 `Transform` 是「脚底 + 无旋转 + 无缩放」的参考系：子节点的局部坐标
//! 才等于世界偏移。父节点被销毁时子节点会一起销毁（`Children` 是 linked spawn）。
//!
//! bevy 0.19：`asset_value(...)` 在场景里现场造材质，`Children [...]` 写子实体树。

use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::prelude::*;

use crate::combat::Faction;
use crate::world::{TerrainConfig, surface_height_at};

use super::components::MainCamera;

/// 玩家精灵（Kenney Tiny Dungeon，CC0）。
pub const PLAYER_SPRITE: &str = "textures/units/player.png";
/// 敌人精灵（同上素材包的幽灵）。
pub const ENEMY_SPRITE: &str = "textures/units/enemy.png";
/// 贴地阴影贴图（程序生成的软边黑圆）。
pub const SHADOW_SPRITE: &str = "textures/units/shadow.png";

/// 精灵纸片的边长（世界单位）：方图配方纸片，像素不会被拉变形。
pub const SPRITE_SIZE: f32 = 1.8;
/// 贴地阴影的直径（世界单位）。
pub const SHADOW_DIAMETER: f32 = 1.9;
/// 阴影缩到最小的参考高度：离地 [`SHADOW_LIFT`] 时只剩 [`SHADOW_MIN_SCALE`]。
pub const SHADOW_LIFT: f32 = 2.0;
/// 离地 [`SHADOW_LIFT`] 时阴影的缩放。
pub const SHADOW_MIN_SCALE: f32 = 0.55;
/// 阴影离地表的微小抬升（世界单位）：避免与体素顶面 z-fighting。
pub const SHADOW_OFFSET: f32 = 0.02;

/// 精灵纸片标记（[`billboard_system`] 每帧对齐相机）。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct UnitSprite;

/// 贴地阴影标记（[`shadow_system`] 每帧贴地 + 随高度收缩）。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct UnitShadow;

/// 单位外观贴图（Startup 预载，组装期取用）。
#[derive(Resource, Debug, Clone)]
pub struct UnitSprites {
    player: Handle<Image>,
    enemy: Handle<Image>,
    shadow: Handle<Image>,
}

impl UnitSprites {
    /// 阵营 → 精灵贴图。
    pub fn sprite(&self, faction: Faction) -> Handle<Image> {
        match faction {
            Faction::Player => self.player.clone(),
            Faction::Enemy => self.enemy.clone(),
        }
    }

    /// 贴地阴影贴图。
    pub fn shadow(&self) -> Handle<Image> {
        self.shadow.clone()
    }
}

/// 预载单位外观（Startup，必须早于 [`AssemblySet`](crate::spawn::AssemblySet) 的组装）。
///
/// 像素图一律用最近邻采样：16px 的原图要拉到 1.8 格，线性过滤会把它糊掉
/// （见 [docs/assets.md](../../../docs/assets.md)）。
pub fn load_unit_sprites(commands: &mut Commands, assets: &AssetServer) {
    let pixel = |path: &'static str| {
        assets
            .load_builder()
            .with_settings(|settings: &mut ImageLoaderSettings| {
                settings.sampler = ImageSampler::nearest();
            })
            .load::<Image>(path)
    };
    commands.insert_resource(UnitSprites {
        player: pixel(PLAYER_SPRITE),
        enemy: pixel(ENEMY_SPRITE),
        shadow: pixel(SHADOW_SPRITE),
    });
}

/// 纸片绕 Y 轴对准相机（billboard）。
///
/// 只转偏航、不抬俯仰：镜头是 40° 左右的斜视角，完全垂直于视线的纸片会在脚下露馅。
/// 贴图的法线是 +Z，绕 Y 转 `atan2(dx, dz)` 就能让正面朝向相机。
pub fn billboard_system(
    camera: Single<&GlobalTransform, With<MainCamera>>,
    mut sprites: Query<(&GlobalTransform, &mut Transform), With<UnitSprite>>,
) {
    let eye = camera.translation();
    for (global, mut transform) in &mut sprites {
        let direction = (eye - global.translation()).with_y(0.0);
        if direction.length_squared() <= f32::EPSILON {
            continue;
        }
        transform.rotation = Quat::from_rotation_y(direction.x.atan2(direction.z));
    }
}

/// 阴影贴到单位正下方的地表，并随离地高度收缩。
///
/// 「离地高度」= 单位的 `Transform.y` 减去它所在 XZ 的地表高度：跳跃、被抬升、
/// 站在矮一层的格子上都会让阴影变小，玩家一眼就能读出脚离地多远。
pub fn shadow_system(
    terrain: Res<TerrainConfig>,
    units: Query<&Transform, (With<Faction>, Without<UnitShadow>)>,
    mut shadows: Query<(&UnitShadow, &ChildOf, &mut Transform), Without<Faction>>,
) {
    for (_, child_of, mut transform) in &mut shadows {
        let Ok(unit) = units.get(child_of.parent()) else {
            continue;
        };
        let ground = surface_height_at(&terrain, unit.translation.x, unit.translation.z) as f32;
        let height = (unit.translation.y - ground).max(0.0);
        let closeness = 1.0 - (height / SHADOW_LIFT).clamp(0.0, 1.0);
        let scale = SHADOW_MIN_SCALE + (1.0 - SHADOW_MIN_SCALE) * closeness;

        // 子节点的局部坐标 = 世界偏移（单位无旋转、无缩放）
        transform.translation = Vec3::new(0.0, ground - unit.translation.y + SHADOW_OFFSET, 0.0);
        transform.scale = Vec3::splat(scale);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::surface_height;

    /// 只装本域系统的 App：相机由测试自己摆，不依赖渲染环境。
    fn sprite_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Update, billboard_system);
        app
    }

    fn shadow_app(terrain: TerrainConfig) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(terrain)
            .add_systems(Update, shadow_system);
        app
    }

    /// 生成「单位 + 它的阴影子节点」，返回两个实体（父、子）。
    fn spawn_unit_with_shadow(app: &mut App, position: Vec3) -> (Entity, Entity) {
        let unit = app
            .world_mut()
            .spawn((Faction::Player, Transform::from_translation(position)))
            .id();
        let shadow = app
            .world_mut()
            .spawn((UnitShadow, Transform::default()))
            .id();
        app.world_mut().entity_mut(unit).add_child(shadow);
        (unit, shadow)
    }

    fn shadow_transform(app: &App, shadow: Entity) -> Transform {
        *app.world().get::<Transform>(shadow).expect("阴影应在场上")
    }

    /// 相机在 +X/+Z 方向：纸片应当转到正面朝它，且不抬头。
    #[test]
    fn billboard_turns_the_sprite_towards_the_camera() {
        let mut app = sprite_app();
        let eye = Vec3::new(12.0, 14.0, 12.0);
        app.world_mut().spawn((
            MainCamera,
            Transform::from_translation(eye),
            GlobalTransform::from_translation(eye),
        ));
        let sprite = app
            .world_mut()
            .spawn((
                UnitSprite,
                Transform::default(),
                GlobalTransform::from_translation(Vec3::ZERO),
            ))
            .id();

        app.update();

        let facing = app.world().get::<Transform>(sprite).unwrap().rotation * Vec3::Z;
        assert!(
            facing.y.abs() < 1e-6,
            "只该转偏航，纸片不能抬头（实际 {facing:?}）"
        );
        let expected = Vec3::new(eye.x, 0.0, eye.z).normalize();
        assert!(
            (facing - expected).length() < 1e-5,
            "纸片正面应当朝相机，实际 {facing:?}"
        );
    }

    /// 站在地上：阴影正好在脚下，满尺寸。
    #[test]
    fn grounded_unit_gets_a_full_sized_shadow_under_its_feet() {
        let terrain = TerrainConfig::default();
        let ground = surface_height(&terrain, 3, 3) as f32;
        let mut app = shadow_app(terrain);
        let (_, shadow) = spawn_unit_with_shadow(&mut app, Vec3::new(3.5, ground, 3.5));

        app.update();

        let transform = shadow_transform(&app, shadow);
        assert!(
            (transform.translation.y - SHADOW_OFFSET).abs() < 1e-6,
            "阴影应当贴在脚下（略抬升免得 z-fighting），实际 {:?}",
            transform.translation
        );
        assert_eq!(transform.scale, Vec3::splat(1.0), "贴地时阴影应当是满尺寸");
    }

    /// 悬空：阴影留在地表、变小，脚与阴影之间的距离就是高度。
    #[test]
    fn raised_unit_gets_a_smaller_shadow_that_stays_on_the_ground() {
        let terrain = TerrainConfig::default();
        let ground = surface_height(&terrain, 3, 3) as f32;
        let mut app = shadow_app(terrain);
        let (_, shadow) = spawn_unit_with_shadow(&mut app, Vec3::new(3.5, ground + 1.0, 3.5));

        app.update();

        let transform = shadow_transform(&app, shadow);
        assert!(
            (transform.translation.y - (-1.0 + SHADOW_OFFSET)).abs() < 1e-6,
            "阴影应当落在地表下方 1 格处，实际 {:?}",
            transform.translation
        );
        let expected = SHADOW_MIN_SCALE + (1.0 - SHADOW_MIN_SCALE) * 0.5;
        assert!(
            (transform.scale.x - expected).abs() < 1e-6,
            "悬空 1 格时阴影应当收缩到 {expected}，实际 {}",
            transform.scale.x
        );
    }

    /// 高到阴影的最小值以后不再继续缩：远处的影子不会缩成一个点。
    #[test]
    fn shadow_scale_stops_at_its_floor() {
        let terrain = TerrainConfig::default();
        let group = surface_height(&terrain, 3, 3) as f32;
        let mut app = shadow_app(terrain);
        let (_, shadow) = spawn_unit_with_shadow(&mut app, Vec3::new(3.5, group + 50.0, 3.5));

        app.update();

        assert_eq!(
            shadow_transform(&app, shadow).scale,
            Vec3::splat(SHADOW_MIN_SCALE),
            "超过参考高度后阴影应当停在最小缩放"
        );
    }

    /// 整机：组装出来的玩家与敌人各带一张**自己阵营**的纸片 + 一个贴地阴影。
    ///
    /// 这条守着「组装层真的把零件挂上去了」——单独的 `unit_scene` 单测看不出
    /// `Children [...]` 有没有写对，而漏挂只会表现成屏幕上少个人。
    #[test]
    fn assembled_units_carry_a_faction_sprite_and_a_ground_shadow() {
        let mut app = crate::test_support::headless_app();
        app.update(); // Startup：组装玩家与敌人
        app.update(); // 表现层跑一帧：阴影贴地

        let sprites = app.world().resource::<UnitSprites>().clone();
        let terrain = *app.world().resource::<TerrainConfig>();

        let mut units = app
            .world_mut()
            .query_filtered::<(Entity, &Faction, &Transform), With<Faction>>();
        let units: Vec<(Entity, Faction, Transform)> = units
            .iter(app.world())
            .map(|(entity, faction, transform)| (entity, *faction, *transform))
            .collect();
        assert_eq!(units.len(), 2, "应当组装出玩家与敌人");

        for (unit, faction, transform) in units {
            let children = app
                .world()
                .get::<Children>(unit)
                .map(|children| children.iter().collect::<Vec<_>>())
                .unwrap_or_default();
            let sprite = children
                .iter()
                .copied()
                .find(|child| app.world().get::<UnitSprite>(*child).is_some())
                .expect("单位应当有一张 2D 纸片");
            let shadow = children
                .iter()
                .copied()
                .find(|child| app.world().get::<UnitShadow>(*child).is_some())
                .expect("单位应当有一个贴地阴影");

            // 纸片：底边落在脚底（根节点原点 = 脚底）
            let sprite_transform = *app.world().get::<Transform>(sprite).unwrap();
            assert_eq!(
                sprite_transform.translation,
                Vec3::new(0.0, SPRITE_SIZE * 0.5, 0.0),
                "纸片应当以底边站在脚底上"
            );

            // 纸片用的是自己阵营的贴图，而不是另一边的
            let material = app
                .world()
                .get::<MeshMaterial3d<StandardMaterial>>(sprite)
                .expect("纸片应当有材质");
            let material = app
                .world()
                .resource::<Assets<StandardMaterial>>()
                .get(&material.0)
                .expect("纸片材质应当已注册");
            assert_eq!(
                material.base_color_texture.as_ref(),
                Some(&sprites.sprite(faction)),
                "{faction:?} 应当用自己阵营的精灵贴图"
            );

            // 阴影：贴在该单位正下方的地表上，且是满尺寸（单位站在地上）
            let shadow_transform = *app.world().get::<Transform>(shadow).unwrap();
            let ground =
                surface_height_at(&terrain, transform.translation.x, transform.translation.z)
                    as f32;
            assert!(
                (shadow_transform.translation.y
                    - (ground - transform.translation.y + SHADOW_OFFSET))
                    .abs()
                    < 1e-6,
                "阴影应当落在地表上，实际 {:?}",
                shadow_transform.translation
            );
            assert_eq!(shadow_transform.scale, Vec3::splat(1.0), "贴地时阴影满尺寸");
        }
    }
}
