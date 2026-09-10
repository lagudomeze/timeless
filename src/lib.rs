//! # app — Project Timeless 世界空间原型（Bevy 0.19）
//!
//! 组织方式：**一个领域 = 一个目录 = 一个 [`Plugin`]**，领域内部按职责分文件
//! （`components` / `events` / `systems` / `resources`），跨领域只经 Bevy `Message`
//! 或公共组件类型通信；输入一律「只翻译、不执行」。
//!
//! | 领域 | 职责 |
//! | :--- | :--- |
//! | [`world`] | 体素地图**数据**：区块、地形生成、体素存取（零渲染依赖，可脱离渲染单测） |
//! | [`voxel_render`] | 体素**表现**：异步网格化、材质、明暗 |
//! | [`movement`] | 速度与位移（位置用 Bevy `Transform`） |
//! | [`combat`] | 生命 / 伤害 / 目标获取 / 攻击实体生命周期 / 技能生成 |
//! | [`ai`] | 敌人决策（只写 `Velocity`） |
//! | [`timeline`] | We-Go 时间线：规划阶段冻结虚拟时间等玩家提交，推进阶段结算行动 |
//! | [`input`] | 玩家输入源（键盘 → 消息，只翻译） |
//! | [`presentation`] | 表现：相机 / 装饰 / 日志（将来还有 UI / 动画 / 特效） |
//! | [`spawn`] | **组装车间**：把各域零件拼成「玩家 / 敌人」实体，含开局组装与重建功能 |
//!
//! 「玩家」「敌人」不是模块，而是组件的组合体——零件归各领域，组装归 [`spawn`]，
//! 且**没有任何领域依赖 `spawn`**：
//!
//! ```text
//! spawn ──▶ combat / movement / ai / world / presentation
//! input ──▶ movement / combat / timeline（只写它们的消息）
//! ai    ──▶ movement / combat（只声明行动实体）
//! ```
//!
//! 跨领域**执行顺序**只在 [`GamePlugin`] 里声明一次，领域内部顺序由各插件自己维护：
//!
//! ```text
//! Startup:  PreloadSet ─▶ AssemblySet
//! Update:   SpawnSet ─▶ InputSet ─▶ TimelineSet ─▶ AiSet ─▶ MovementSet ─▶ CombatSet
//!           ─▶ VoxelRenderSet ─▶ PresentationSet
//! WorldSet ────────────────────────▶（必须早于 VoxelRenderSet）
//! ```

use bevy::prelude::*;

pub mod ai;
pub mod combat;
pub mod input;
pub mod movement;
pub mod presentation;
pub mod spawn;
pub mod timeline;
pub mod voxel_render;
pub mod world;

pub use ai::{AiPlugin, AiSet};
pub use combat::{CombatPlugin, CombatSet};
pub use input::{InputPlugin, InputSet};
pub use movement::{MovementPlugin, MovementSet};
pub use presentation::{PreloadSet, PresentationPlugin, PresentationSet};
pub use spawn::{AssemblySet, SpawnPlugin, SpawnSet};
pub use timeline::{TimelinePlugin, TimelineSet};
pub use voxel_render::{VoxelRenderPlugin, VoxelRenderSet};
pub use world::{WorldPlugin, WorldSet};

/// 游戏装配插件：把各领域插件按流水线接起来。
///
/// `main.rs` 只加引擎插件 + 本插件；领域之间谁先谁后只在这里说一次。
#[derive(Debug, Default)]
pub struct GamePlugin;

/// 声明跨领域执行顺序（领域内部的顺序由各插件自己维护）。
///
/// [`GamePlugin`] 与单元测试共用这一个入口，保证测试跑的就是真实流水线顺序；
/// 只装了部分领域的 App 也能安全调用（空系统集不产生任何影响）。
pub fn configure_pipeline(app: &mut App) {
    app.configure_sets(Startup, (PreloadSet, AssemblySet).chain())
        .configure_sets(
            Update,
            (
                SpawnSet,
                InputSet,
                TimelineSet,
                AiSet,
                MovementSet,
                CombatSet,
                VoxelRenderSet,
                PresentationSet,
            )
                .chain(),
        )
        // 数据先于表现：区块先有数据，网格化才有东西可画
        .configure_sets(Update, WorldSet.before(VoxelRenderSet));
}

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        configure_pipeline(app);
        app.add_plugins((
            WorldPlugin,
            VoxelRenderPlugin,
            PresentationPlugin,
            SpawnPlugin,
            InputPlugin,
            TimelinePlugin,
            MovementPlugin,
            CombatPlugin,
            AiPlugin,
        ));
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use bevy::scene::ScenePlugin;
    use bevy::world_serialization::WorldSerializationPlugin;

    /// 装齐「除渲染外」的整机 App：跨领域行为测试用（时间线、重置、场景组装）。
    ///
    /// 不含 `VoxelRenderPlugin`（网格化要 `Assets<Mesh>`，体素那侧自己搭 App）。
    pub fn headless_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .add_plugins((ScenePlugin, WorldSerializationPlugin))
            .insert_resource(ButtonInput::<bevy::input::keyboard::KeyCode>::default())
            .insert_resource(ButtonInput::<bevy::input::mouse::MouseButton>::default())
            .insert_resource(bevy::input::mouse::AccumulatedMouseMotion::default())
            .add_plugins((
                WorldPlugin,
                PresentationPlugin,
                SpawnPlugin,
                InputPlugin,
                TimelinePlugin,
                MovementPlugin,
                CombatPlugin,
                AiPlugin,
            ));
        configure_pipeline(&mut app);
        app
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::keyboard::KeyCode;
    use bevy::scene::ScenePlugin;
    use bevy::time::TimeUpdateStrategy;
    use bevy::world_serialization::WorldSerializationPlugin;
    use std::time::Duration;

    use crate::combat::{
        Armor, Collidable, Faction, HitOnce, HitRadius, Lifetime, MeleeShape, PhysicalDamage,
        Projectile, health::Health,
    };
    use crate::movement::{MoveSpeed, Velocity};
    use crate::timeline::{Declared, Timeline};

    /// 最小 App：装输入 / 时间线 / 战斗 / 移动领域，不启动渲染。
    ///
    /// 时间用 `ManualDuration` 手动步进，测试因此可以精确走完「声明 → 提交 →
    /// 到点执行 → 窗口结束」的整条时间线。
    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .add_plugins((ScenePlugin, WorldSerializationPlugin))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(ButtonInput::<bevy::input::mouse::MouseButton>::default())
            .insert_resource(bevy::input::mouse::AccumulatedMouseMotion::default())
            // 输入域写的消息由「消费它们的领域」注册；轻量 App 里手动补上相机平移
            .add_message::<crate::presentation::PanCamera>()
            .add_plugins((InputPlugin, TimelinePlugin, MovementPlugin, CombatPlugin));
        configure_pipeline(&mut app);
        app
    }

    fn velocity_of(app: &mut App, entity: Entity) -> Vec3 {
        app.world().get::<Velocity>(entity).unwrap().0
    }

    fn declared_actions(app: &mut App) -> usize {
        let mut query = app.world_mut().query_filtered::<Entity, With<Declared>>();
        query.iter(app.world()).count()
    }

    #[test]
    fn arrow_damages_target_then_is_cleaned_up() {
        let mut app = test_app();
        let target = app
            .world_mut()
            .spawn((
                Health::new(100.0),
                Collidable,
                HitRadius(0.8),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id();
        let arrow = app
            .world_mut()
            .spawn((
                Velocity(Vec3::ZERO),
                Projectile::default(),
                HitRadius(0.2),
                PhysicalDamage(10.0),
                Transform::from_xyz(0.5, 0.0, 0.0),
            ))
            .id();

        app.update(); // 碰撞挂标记
        app.update(); // 伤害 → 命中结束 → 清理

        let world = app.world_mut();
        assert!(
            world.get_entity(arrow).is_err(),
            "普通射弹（穿透 1）命中后应被清理"
        );
        let hp = world.query::<&Health>().get(world, target).unwrap();
        assert_eq!(hp.current, 90.0, "10 点物理伤害应扣减 10 点生命");
    }

    #[test]
    fn armor_reduces_physical_damage() {
        let mut app = test_app();
        let target = app
            .world_mut()
            .spawn((
                Health::new(100.0),
                Collidable,
                HitRadius(0.8),
                Armor(3.0),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id();
        app.world_mut().spawn((
            Velocity(Vec3::ZERO),
            Projectile::default(),
            HitRadius(0.2),
            PhysicalDamage(10.0),
            Transform::from_xyz(0.5, 0.0, 0.0),
        ));

        app.update();
        app.update();

        let world = app.world_mut();
        let hp = world.query::<&Health>().get(world, target).unwrap();
        assert_eq!(hp.current, 93.0, "10 点物理伤害应被 3 点护甲减免");
    }

    #[test]
    fn lethal_damage_triggers_despawn() {
        let mut app = test_app();
        let target = app
            .world_mut()
            .spawn((
                Health::new(5.0),
                Collidable,
                HitRadius(0.8),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id();
        app.world_mut().spawn((
            Velocity(Vec3::ZERO),
            Projectile::default(),
            HitRadius(0.2),
            PhysicalDamage(10.0),
            Transform::from_xyz(0.5, 0.0, 0.0),
        ));

        app.update();
        app.update();

        assert!(
            app.world_mut().get_entity(target).is_err(),
            "致命伤害应触发 DeathEvent 并销毁目标"
        );
    }

    #[test]
    fn wasd_declares_a_move_then_enter_commits_the_round() {
        let mut app = test_app();
        let player = app
            .world_mut()
            .spawn((
                Faction::Player,
                Velocity(Vec3::ZERO),
                MoveSpeed(5.0),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id();

        // 规划阶段：虚拟时间冻结，按键只**声明**行动，不产生位移
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        app.update();
        assert!(
            app.world().resource::<Timeline>().is_planning(),
            "没有提交时应当停在规划阶段"
        );
        assert_eq!(declared_actions(&mut app), 1, "W 应当声明一条移动草案");
        assert_eq!(velocity_of(&mut app, player), Vec3::ZERO, "冻结期间不移动");
        assert!(
            app.world().resource::<Time<Virtual>>().is_paused(),
            "规划阶段虚拟时间必须暂停"
        );

        // Enter 提交 → 进入推进阶段，草案变成待执行行动
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        // 只提交这一次：清掉按键状态，避免后面每帧重复提交
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        assert!(
            !app.world().resource::<Timeline>().is_planning(),
            "提交后应开始推进"
        );
        assert_eq!(declared_actions(&mut app), 0, "草案应全部转为待执行");
        assert!(
            !app.world().resource::<Time<Virtual>>().is_paused(),
            "推进阶段虚拟时间必须恢复流动"
        );

        // 推进：前摇 0.15s，100ms/帧 → 两帧之内落地
        let mut moved = false;
        for _ in 0..3 {
            app.update();
            if velocity_of(&mut app, player).z > 0.0 {
                moved = true;
                break;
            }
        }
        assert!(moved, "到点后执行器应给玩家 +Z 速度（地面，不是天上）");
        let velocity = velocity_of(&mut app, player);
        assert_eq!(velocity.y, 0.0, "平面移动不该产生竖直速度");

        // 窗口（1s）走完 → 世界重新冻结并广播 RoundEnded
        for _ in 0..12 {
            app.update();
        }
        assert!(
            app.world().resource::<Timeline>().is_planning(),
            "窗口结束应回到规划阶段等待下一次提交"
        );
        assert_eq!(
            velocity_of(&mut app, player),
            Vec3::ZERO,
            "本轮结束单位应当停下"
        );
    }

    /// 整机回归：用真实斜视角机位跑一轮，按 W 必须贴地走向远处，而不是飞上天。
    #[test]
    fn committed_move_stays_on_the_ground_and_follows_the_camera() {
        let mut app = crate::test_support::headless_app();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            100,
        )));
        app.update(); // Startup：组装单位 + 相机

        let (player, start) = {
            let mut query = app.world_mut().query::<(Entity, &Faction, &Transform)>();
            query
                .iter(app.world())
                .find(|(_, faction, _)| **faction == Faction::Player)
                .map(|(entity, _, transform)| (entity, transform.translation))
                .expect("应当有玩家")
        };
        let camera_forward = {
            let mut query = app
                .world_mut()
                .query_filtered::<&Transform, With<crate::presentation::CameraRig>>();
            let transform = *query.iter(app.world()).next().expect("应当有相机");
            (transform.rotation * Vec3::NEG_Z).with_y(0.0).normalize()
        };

        // 按住 W（屏幕向上 = 远离相机）并提交本轮
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();

        for _ in 0..6 {
            app.update();
        }

        let moved = app.world().get::<Transform>(player).unwrap().translation - start;
        assert!(
            moved.y.abs() < 1e-3,
            "移动必须留在地面上，实际位移 {moved:?}"
        );
        let ground = Vec2::new(moved.x, moved.z);
        assert!(ground.length() > 0.5, "应当真的走了一段：{moved:?}");
        let forward = Vec2::new(camera_forward.x, camera_forward.z).normalize();
        assert!(
            ground.normalize().dot(forward) > 0.9,
            "W 应当朝远离相机的方向走：{moved:?}"
        );
    }

    #[test]
    fn declaration_replaces_the_previous_draft() {
        let mut app = test_app();
        app.world_mut().spawn((
            Faction::Player,
            Velocity(Vec3::ZERO),
            MoveSpeed(5.0),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ));
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::KeyW);
        app.update();

        // 同一轮里改声明：射击覆盖移动（一个单位同时只有一个行动）
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.release(KeyCode::KeyW);
        input.press(KeyCode::KeyQ);
        app.update();

        assert_eq!(declared_actions(&mut app), 1, "同一轮只允许一个草案");
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<crate::combat::skills::ShootAction>>()
                .iter(app.world())
                .count(),
            1,
            "后声明的技能应当顶掉移动草案"
        );
    }

    #[test]
    fn held_movement_key_keeps_a_declared_skill() {
        let mut app = test_app();
        app.world_mut().spawn((
            Faction::Player,
            Velocity(Vec3::ZERO),
            MoveSpeed(5.0),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ));

        // 按住 W 移动，再按 Space 声明射击
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyQ);
        app.update();

        fn shoot_drafts(app: &mut App) -> usize {
            app.world_mut()
                .query_filtered::<Entity, With<crate::combat::skills::ShootAction>>()
                .iter(app.world())
                .count()
        }
        assert_eq!(shoot_drafts(&mut app), 1, "Space 应当声明射击");

        // Space 已经松开（只按过一次），W 仍按住：移动输入没变化
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.clear();
        input.press(KeyCode::KeyW);
        app.update();
        assert_eq!(
            shoot_drafts(&mut app),
            1,
            "按住移动键不应把已声明的技能草案顶掉"
        );
        assert_eq!(declared_actions(&mut app), 1, "同一轮只允许一个草案");
    }

    /// 空格声明跳跃：到点后离地，并在窗口内落回起跳高度。
    #[test]
    fn space_declares_a_jump_that_leaves_and_returns_to_the_ground() {
        let mut app = test_app();
        let player = app
            .world_mut()
            .spawn((
                Faction::Player,
                Velocity(Vec3::ZERO),
                MoveSpeed(5.0),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id();

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<crate::movement::JumpAction>>()
                .iter(app.world())
                .count(),
            1,
            "空格应当声明一条跳跃草案"
        );

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();

        let mut peak = 0.0f32;
        for _ in 0..5 {
            app.update();
            peak = peak.max(app.world().get::<Transform>(player).unwrap().translation.y);
        }
        assert!(peak > 0.3, "跳跃应当离地，实际最高 {peak}");

        for _ in 0..6 {
            app.update();
        }
        assert_eq!(
            app.world().get::<Transform>(player).unwrap().translation.y,
            0.0,
            "应当落回起跳高度"
        );
        assert!(
            app.world()
                .get::<crate::movement::Jumping>(player)
                .is_none(),
            "落地后应当移除跳跃状态"
        );
    }

    #[test]
    fn melee_swing_hits_once_then_expires() {
        let mut app = test_app();
        let enemy = app
            .world_mut()
            .spawn((
                Faction::Enemy,
                Health::new(50.0),
                Collidable,
                HitRadius(0.8),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id();
        let swing = app
            .world_mut()
            .spawn((
                Faction::Player,
                PhysicalDamage(15.0),
                MeleeShape::default(),
                HitOnce::default(),
                Lifetime::default(),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id();

        app.update(); // 命中结算

        let world = app.world_mut();
        let hp = world.query::<&Health>().get(world, enemy).unwrap();
        assert_eq!(hp.current, 35.0, "近战 15 点伤害只应结算一次");

        // 直接让计时器到期，验证 Lifetime 销毁（避免依赖测试时间推进）
        app.world_mut()
            .entity_mut(swing)
            .get_mut::<Lifetime>()
            .unwrap()
            .0
            .set_elapsed(Duration::from_secs(1));
        app.update();

        assert!(
            app.world_mut().get_entity(swing).is_err(),
            "近战攻击实体应在 Lifetime 结束后销毁"
        );
    }
}
