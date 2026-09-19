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
//! | [`movement`] | 格子决策（`Cell` / `MoveGoal`）+ 速度位移 + 移动 / 跳跃 / 翻滚行动 |
//! | [`combat`] | 生命 / 伤害 / 目标获取 / 攻击实体生命周期 / 技能 / 精力 / 防御 / 反应 |
//! | [`ai`] | 敌人决策（选意图 → 声明行动，同样不改状态） |
//! | [`timeline`] | 无回合调度：决策槽决定谁能决策，每个动作自带前摇 + 后摇 |
//! | [`input`] | 玩家输入源（键盘 / 鼠标 → 消息，只翻译） |
//! | [`interaction`] | 鼠标交互：射线拾取、网格高亮、点击 → 消息、行动预演指示器 |
//! | [`presentation`] | 表现：相机 / 单位纸片与贴地阴影 / HUD / 装饰 / 日志 |
//! | [`spawn`] | **组装车间**：把各域零件拼成「玩家 / 敌人」实体，含开局组装与重置 |
//!
//! 「玩家」「敌人」不是模块，而是组件的组合体——零件归各领域，组装归 [`spawn`]，
//! 且**没有任何领域依赖 `spawn`**：
//!
//! ```text
//! spawn ──▶ combat / movement / ai / world / presentation
//! input ──▶ movement / combat / timeline / interaction / presentation / spawn（只写它们的消息）
//! interaction ──▶ movement / combat / timeline（点击解释成它们的消息）
//! ai    ──▶ movement / combat（只声明行动实体）
//! ```
//!
//! 跨领域**执行顺序**只在 [`configure_pipeline`] 里声明一次，领域内部顺序由各插件自己维护：
//!
//! ```text
//! Startup:  PreloadSet ─▶ AssemblySet
//! Update:   SpawnSet ─▶ InputSet ─▶ InteractionSet ─▶ TimelineSet ─▶ AiSet
//!           ─▶ MovementSet ─▶ CombatSet ─▶ VoxelRenderSet ─▶ PresentationSet ─▶ ClockSet
//! WorldSet ────────────────────────▶（必须早于 VoxelRenderSet）
//! ```
//!
//! [`ClockSet`] 排在帧末：这一帧所有系统看到的是同一个冻结状态，而**唯一**写
//! `Time<Virtual>` 的 `apply_clock` 就在那里落地（见 [`timeline`]）。

use bevy::prelude::*;

pub mod ai;
pub mod combat;
pub mod input;
pub mod interaction;
pub mod movement;
pub mod presentation;
pub mod spawn;
pub mod timeline;
pub mod voxel_render;
pub mod world;

pub use ai::{AiPlugin, AiSet};
pub use combat::{CombatPlugin, CombatSet};
pub use input::{InputPlugin, InputSet};
pub use interaction::{InteractionPlugin, InteractionSet};
pub use movement::{MovementPlugin, MovementSet};
pub use presentation::{PreloadSet, PresentationPlugin, PresentationSet};
pub use spawn::{AssemblySet, SpawnPlugin, SpawnSet};
pub use timeline::{ClockSet, TimelinePlugin, TimelineSet};
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
                InteractionSet,
                TimelineSet,
                AiSet,
                MovementSet,
                CombatSet,
                VoxelRenderSet,
                PresentationSet,
                // 帧末：暂停请求 → 原因集合 → 时钟。冻结只影响下一帧
                ClockSet,
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
            InteractionPlugin,
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
            // Bevy 的输入插件负责每帧清空 `just_pressed`：缺了它，一次按键会被
            // 当成「永远刚按下」（测试里表现为按一次就走无数格）
            .add_plugins(bevy::input::InputPlugin)
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            // `setup_hud` 会 `AssetServer::load` 一份字体句柄；不注册 `Font` 资产类型
            // 会在 `load` 那一刻 panic（而且是在并行计算线程里，错误信息很难指向 HUD）
            .init_asset::<Font>()
            // 单位精灵同理：`presentation::preload` 会 load 三张贴图
            .init_asset::<Image>()
            .insert_resource(ButtonInput::<bevy::input::keyboard::KeyCode>::default())
            .insert_resource(ButtonInput::<bevy::input::mouse::MouseButton>::default())
            .insert_resource(bevy::input::mouse::AccumulatedMouseMotion::default())
            .add_plugins((
                WorldPlugin,
                PresentationPlugin,
                SpawnPlugin,
                InputPlugin,
                // 输入域要写的 `PointerCommand` 由交互域注册
                InteractionPlugin,
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

    use crate::combat::defense::{Dodging, Parrying, ROLL_COST, RollCommand, Stamina};
    use crate::combat::skills::{FireballAction, fireball_action_scene};
    use crate::combat::{
        Armor, Collidable, DamageEvent, Faction, Fireball, HitOnce, HitRadius, InterruptPower,
        Lifetime, MeleeShape, PhysicalDamage, Projectile, Threatens, health::Health,
    };
    use crate::movement::{Cell, Jumping, MoveAction, MoveSpeed, Velocity};
    use crate::timeline::{
        DecisionSlot, FOCUS_MAX, Focus, InputDriven, InterruptEvent, PauseReasons, ScheduledAction,
        THREAT, UndoCommand, timing,
    };
    use crate::world::{TerrainConfig, ground_position};

    /// 最小 App：装输入 / 时间线 / 战斗 / 移动领域，不启动渲染。
    ///
    /// 时间用 `ManualDuration` 手动步进（100ms/帧），测试因此可以精确走完
    /// 「声明 → 前摇到点 → 执行器落地 → 后摇恢复」的整条时间线。
    ///
    /// 注册 `Mesh` / `StandardMaterial` 两个资产类型：行动实体的场景工厂
    /// （火球 / 近战横扫）用 `asset_value(...)` 造视觉，而 BSN 模板在实例化时
    /// 要读对应的 `Assets<R>` 资源。不注册就会在 `spawn_scene` 里 panic。
    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .add_plugins((ScenePlugin, WorldSerializationPlugin))
            // InputPlugin 会每帧清空 `just_pressed`：没有它，一次按键会被当成「永远刚按下」
            .add_plugins(bevy::input::InputPlugin)
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            // HUD 会加载字体句柄，因此测试 App 也要注册 `Font` 资产类型
            .init_asset::<Font>()
            // 贴地系统要读地形高度：测试 App 少了它会在系统初始化就 panic
            .init_resource::<TerrainConfig>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(ButtonInput::<bevy::input::mouse::MouseButton>::default())
            .insert_resource(bevy::input::mouse::AccumulatedMouseMotion::default())
            // 输入域写的消息由「消费它们的领域」注册；轻量 App 里手动补上相机平移
            .add_message::<crate::presentation::PanCamera>()
            // 同上：F1 的帮助开关由 HUD 消费，这里没有 PresentationPlugin
            .add_message::<crate::presentation::ToggleHelp>()
            // 预演读数也是 HUD 消费的消息（写方是 interaction）
            .add_message::<crate::presentation::hud::PreviewReadout>()
            // 滚轮缩放由 presentation 消费（本测试 App 没有 PresentationPlugin）
            .add_message::<crate::presentation::ZoomCamera>()
            // F5 重置由 spawn 消费（本测试 App 没有 SpawnPlugin）
            .add_message::<crate::spawn::ResetBattle>()
            .add_plugins((
                InputPlugin,
                // 输入域要写的 `PointerCommand` 由交互域注册
                InteractionPlugin,
                TimelinePlugin,
                MovementPlugin,
                CombatPlugin,
            ));
        configure_pipeline(&mut app);
        app
    }

    fn velocity_of(app: &mut App, entity: Entity) -> Vec3 {
        app.world().get::<Velocity>(entity).unwrap().0
    }

    /// 场上还没被收拾掉的行动实体数（按载荷类型数）。
    fn actions<A: Component>(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<A>>()
            .iter(app.world())
            .count()
    }

    fn slot_of(app: &App, entity: Entity) -> DecisionSlot {
        app.world()
            .get::<DecisionSlot>(entity)
            .copied()
            .expect("单位应当有决策槽")
    }

    /// 按下一个键（走 Bevy 真正的输入管线）。
    ///
    /// 直接改 `ButtonInput` 不行：装了 `bevy::input::InputPlugin` 之后，
    /// 每帧的 `keyboard_input_system` 会用键盘事件重建按钮状态，
    /// 手改的值会在下一次 `PreUpdate` 被清掉。所以这里投递真实的
    /// [`KeyboardInput`](bevy::input::keyboard::KeyboardInput) 事件。
    fn press(app: &mut App, key: KeyCode) {
        app.world_mut()
            .write_message(bevy::input::keyboard::KeyboardInput {
                key_code: key,
                logical_key: bevy::input::keyboard::Key::Unidentified(
                    bevy::input::keyboard::NativeKey::Unidentified,
                ),
                state: bevy::input::ButtonState::Pressed,
                repeat: false,
                window: Entity::PLACEHOLDER,
                text: None,
            });
    }

    /// 玩家单位：零件与 [`crate::spawn::player_scene`] 对齐。
    ///
    /// 少了 `InputDriven`，时间线就当场上没有玩家——世界不会为谁停下来，
    /// 反应系统也不知道该保护谁。
    fn spawn_player(app: &mut App, cell: Cell, world: Vec3) -> Entity {
        app.world_mut()
            .spawn((
                Faction::Player,
                Health::new(50),
                Collidable,
                HitRadius(0.8),
                Velocity(Vec3::ZERO),
                MoveSpeed(5.0),
                Stamina::default(),
                cell,
                DecisionSlot::Empty,
                InputDriven,
                Transform::from_translation(world),
            ))
            .id()
    }

    /// 敌人单位：同样的骨架，但没有 `InputDriven`（时间线不为它停表）。
    fn spawn_enemy(app: &mut App, cell: Cell, world: Vec3) -> Entity {
        app.world_mut()
            .spawn((
                Faction::Enemy,
                Health::new(50),
                Collidable,
                HitRadius(0.8),
                Velocity(Vec3::ZERO),
                MoveSpeed(2.0),
                Stamina::default(),
                cell,
                DecisionSlot::Empty,
                Transform::from_translation(world),
            ))
            .id()
    }

    #[test]
    fn arrow_damages_target_then_is_cleaned_up() {
        let mut app = test_app();
        let target = app
            .world_mut()
            .spawn((
                Health::new(100),
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
                PhysicalDamage(10),
                Transform::from_xyz(0.5, 0.0, 0.0),
            ))
            .id();

        app.update(); // 碰撞挂标记 → 命中结算
        app.update(); // 清理

        let world = app.world_mut();
        assert!(
            world.get_entity(arrow).is_err(),
            "普通射弹（穿透 1）命中后应被清理"
        );
        let hp = world.query::<&Health>().get(world, target).unwrap();
        assert_eq!(hp.current, 90, "10 点物理伤害应扣减 10 点生命");
    }

    #[test]
    fn armor_reduces_physical_damage() {
        let mut app = test_app();
        let target = app
            .world_mut()
            .spawn((
                Health::new(100),
                Collidable,
                HitRadius(0.8),
                Armor(3),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id();
        app.world_mut().spawn((
            Velocity(Vec3::ZERO),
            Projectile::default(),
            HitRadius(0.2),
            PhysicalDamage(10),
            Transform::from_xyz(0.5, 0.0, 0.0),
        ));

        app.update();
        app.update();

        let world = app.world_mut();
        let hp = world.query::<&Health>().get(world, target).unwrap();
        assert_eq!(hp.current, 93, "10 点物理伤害应被 3 点护甲减免");
    }

    #[test]
    fn lethal_damage_triggers_despawn() {
        let mut app = test_app();
        let target = app
            .world_mut()
            .spawn((
                Health::new(5),
                Collidable,
                HitRadius(0.8),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id();
        app.world_mut().spawn((
            Velocity(Vec3::ZERO),
            Projectile::default(),
            HitRadius(0.2),
            PhysicalDamage(10),
            Transform::from_xyz(0.5, 0.0, 0.0),
        ));

        app.update();
        app.update();

        assert!(
            app.world_mut().get_entity(target).is_err(),
            "致命伤害应触发 DeathEvent 并销毁目标"
        );
    }

    /// 无回合模型：按一次方向键走**一格**，走到格中心自动停下并更新 `Cell`。
    ///
    /// 节奏（100ms/帧）：声明 → 前摇 0.15s → 位移 0.4s → 后摇走完清空决策槽。
    #[test]
    fn pressing_walks_exactly_one_cell_and_stops_at_its_center() {
        let mut app = test_app();
        let player = spawn_player(&mut app, Cell::new(0, 0), Vec3::ZERO);

        press(&mut app, KeyCode::ArrowUp);
        app.update(); // 声明：行动实体 + 决策槽 Filled
        assert_eq!(actions::<MoveAction>(&mut app), 1, "按一次应当产生一条移动");
        assert_eq!(
            slot_of(&app, player),
            DecisionSlot::Windup,
            "声明之后决策槽进入前摇"
        );
        assert_eq!(
            velocity_of(&mut app, player),
            Vec3::ZERO,
            "前摇未到时不该有速度"
        );

        for _ in 0..12 {
            app.update();
        }

        assert_eq!(
            app.world().get::<Cell>(player).copied(),
            Some(Cell::new(0, 1)),
            "方向键应当走一格到 +Z 方向的邻格"
        );
        // 目标格 (0,1) 的中心 = 世界 (1, 3)；`y` 由地形决定（停下时贴地）
        let terrain = *app.world().resource::<TerrainConfig>();
        let expected = ground_position(&terrain, 1.0, 3.0);
        let transform = app.world().get::<Transform>(player).unwrap().translation;
        assert_eq!(
            (transform.x, transform.z),
            (expected.x, expected.z),
            "应当停在目标格中心（格 (0,1) 的中心）"
        );
        assert_eq!(
            transform.y, expected.y,
            "停下时应当贴着目标格的地面，而不是停在出生高度"
        );
        assert_eq!(
            velocity_of(&mut app, player),
            Vec3::ZERO,
            "到格中心应当停下"
        );
    }

    /// **没有「确认」这一步**：声明即生效，不需要按 Enter。
    #[test]
    fn fast_mode_applies_input_without_enter() {
        let mut app = test_app();
        let player = spawn_player(&mut app, Cell::new(0, 0), Vec3::ZERO);

        press(&mut app, KeyCode::ArrowUp);
        for _ in 0..12 {
            app.update();
        }

        assert_eq!(
            app.world().get::<Cell>(player).copied(),
            Some(Cell::new(0, 1)),
            "没有按 Enter，方向键也应当直接走一格"
        );
    }

    /// 后摇走完：决策槽清空，又能声明下一手。
    #[test]
    fn recovery_restores_the_ability_to_decide() {
        let mut app = test_app();
        let player = spawn_player(&mut app, Cell::new(0, 0), Vec3::ZERO);

        press(&mut app, KeyCode::ArrowUp);
        for _ in 0..20 {
            app.update();
        }

        assert_eq!(
            slot_of(&app, player),
            DecisionSlot::Empty,
            "后摇结束应当清空决策槽"
        );
    }

    /// 整机回归：用真实斜视角机位跑一次决策，按方向键必须贴地走到**相邻格中心**，
    /// 而不是飞上天，也不该停在格角上。
    #[test]
    fn move_stays_on_the_ground_and_follows_the_camera() {
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

        // 按上（屏幕向上）：一次决策走一格，方向 = 相机前方（远离相机）
        press(&mut app, KeyCode::ArrowUp);
        for _ in 0..14 {
            app.update();
        }

        let end = app.world().get::<Transform>(player).unwrap().translation;
        let cell = app
            .world()
            .get::<Cell>(player)
            .copied()
            .expect("应当有格子");
        // 贴地：结束位置的高度必须是**脚下那格**的地表高度
        let terrain = *app.world().resource::<TerrainConfig>();
        let ground = ground_position(&terrain, end.x, end.z).y;
        assert_eq!(end.y, ground, "走完应当贴着地面");
        let moved = (end - start).with_y(0.0);
        assert!(
            moved.dot(camera_forward) > 0.0,
            "方向键应当朝相机前方走，实际位移 {moved:?}"
        );
        let center = cell.center();
        assert!(
            (end.x - center.x).abs() < 1e-3 && (end.z - center.y).abs() < 1e-3,
            "应当停在格子中心，实际 {end:?} vs 格 {center:?}"
        );
    }

    #[test]
    fn melee_swing_hits_once_then_expires() {
        let mut app = test_app();
        let enemy = app
            .world_mut()
            .spawn((
                Faction::Enemy,
                Health::new(50),
                Collidable,
                HitRadius(0.8),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id();
        let swing = app
            .world_mut()
            .spawn((
                Faction::Player,
                PhysicalDamage(15),
                MeleeShape::default(),
                HitOnce::default(),
                Lifetime::default(),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id();

        app.update(); // 命中结算

        let world = app.world_mut();
        let hp = world.query::<&Health>().get(world, enemy).unwrap();
        assert_eq!(hp.current, 35, "近战 15 点伤害只应结算一次");

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

    /// 翻滚：花 1 点精力、退一格、进入无敌帧（`Dodging`）。
    #[test]
    fn roll_spends_stamina_and_grants_invulnerability() {
        let mut app = test_app();
        let player = spawn_player(&mut app, Cell::new(0, 0), Vec3::ZERO);
        app.world_mut().get_mut::<Stamina>(player).unwrap().current = 3;
        assert_eq!(ROLL_COST, 1, "这条测试按 1 点精力写着");
        // 威胁在东侧：翻滚应当朝西退（-X）
        spawn_enemy(&mut app, Cell::new(5, 0), Vec3::new(10.0, 0.0, 0.0));

        app.world_mut().write_message(RollCommand);
        // 翻滚落地那一帧：精力被扣掉，且**还没**开始回复。
        //
        // 必须在这一刻观测：`recovery_system` 会在后摇结束时回 1 点精力
        // （`ROLL.recovery = 0.30`），再往后看就分不清「没扣」和「扣了又回了」。
        let mut spent_on_arrival = None;
        for _ in 0..8 {
            app.update();
            if app.world().get::<Dodging>(player).is_some() {
                spent_on_arrival = app.world().get::<Stamina>(player).map(|s| s.current);
                break;
            }
        }

        assert!(
            app.world().get::<Dodging>(player).is_some(),
            "翻滚应当挂上无敌帧标记"
        );
        assert_eq!(
            spent_on_arrival,
            Some(2),
            "翻滚落地时应当从 3 点精力里扣掉 1 点"
        );
        // 后摇走完：精力回到上限（每次重新可决策回复 1 点）
        for _ in 0..12 {
            app.update();
        }
        assert_eq!(
            app.world().get::<Stamina>(player).unwrap().current,
            3,
            "后摇结束重新可决策时应当回复 1 点精力"
        );
        let moved = app.world().get::<Transform>(player).unwrap().translation;
        assert!(moved.x < 0.0, "应当朝远离威胁的方向退一格，实际 {moved:?}");
    }

    /// 招架：挡下绑定的那次攻击，并把一半伤害反制回攻击者。
    #[test]
    fn parry_negates_the_bound_attack_and_counters() {
        let mut app = test_app();
        let player = app
            .world_mut()
            .spawn((
                Faction::Player,
                Health::new(50),
                Collidable,
                HitRadius(0.8),
                Stamina::new(3),
                Cell::new(0, 0),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id();
        let attacker = app
            .world_mut()
            .spawn((
                Faction::Enemy,
                Health::new(50),
                PhysicalDamage(15),
                MeleeShape::default(),
                HitOnce::default(),
                Lifetime::default(),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id();
        app.world_mut().entity_mut(player).insert(Parrying {
            target_attack: attacker,
            expires_at: 999.0,
        });

        app.update(); // 目标获取 → 防御判定 → 扣血

        assert_eq!(
            app.world().get::<Health>(player).unwrap().current,
            50,
            "招架应当完全免伤"
        );
        assert_eq!(
            app.world().get::<Health>(attacker).unwrap().current,
            42,
            "招架应当把 15 点的一半（向上取整）反制回去"
        );
    }

    /// 无敌帧：翻滚期间命中被闪开，一段时间后失效。
    #[test]
    fn dodging_negates_damage_until_it_expires() {
        let mut app = test_app();
        let player = app
            .world_mut()
            .spawn((
                Faction::Player,
                Health::new(50),
                Collidable,
                HitRadius(0.8),
                Cell::new(0, 0),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id();
        // 先验证无敌帧还在时：20 点伤害应当被完全闪开
        app.world_mut().spawn((
            Faction::Enemy,
            PhysicalDamage(20),
            MeleeShape::default(),
            HitOnce::default(),
            Lifetime::default(),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ));
        app.world_mut()
            .entity_mut(player)
            .insert(Dodging { expires_at: 999.0 });

        app.update();
        assert_eq!(
            app.world().get::<Health>(player).unwrap().current,
            50,
            "无敌帧内应当完全闪开"
        );

        // 让无敌帧立刻过期，再打一次
        app.world_mut()
            .entity_mut(player)
            .get_mut::<Dodging>()
            .unwrap()
            .expires_at = 0.0;
        app.world_mut().spawn((
            Faction::Enemy,
            PhysicalDamage(20),
            MeleeShape::default(),
            HitOnce::default(),
            Lifetime::default(),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ));
        app.update();
        assert_eq!(
            app.world().get::<Health>(player).unwrap().current,
            30,
            "无敌帧过期后应当照常吃伤害"
        );
    }

    /// 跳跃：弹道结束后落回起跳高度，不会卡在空中。
    #[test]
    fn jump_does_not_get_stuck_in_the_air() {
        let mut app = test_app();
        let player = spawn_player(&mut app, Cell::new(0, 0), Vec3::ZERO);
        press(&mut app, KeyCode::KeyC);
        for _ in 0..12 {
            app.update();
        }
        assert!(
            app.world().get::<Jumping>(player).is_none(),
            "跳跃应当落地并清掉状态"
        );
    }

    /// 火球：锁格飞行 → 到达目标格 → 按**真实距离**结算 AoE。
    #[test]
    fn fireball_flies_to_the_locked_cell_and_explodes() {
        let mut app = test_app();
        spawn_player(&mut app, Cell::new(0, 0), Vec3::new(1.0, 0.0, 1.0));
        // 目标格 (2,0) 的中心 = (5, 0, 1)：火球应当飞到那里再炸
        let enemy = spawn_enemy(&mut app, Cell::new(2, 0), Vec3::new(5.0, 0.0, 1.0));

        press(&mut app, KeyCode::KeyQ);
        for _ in 0..18 {
            app.update();
        }

        assert_eq!(
            app.world().get::<Health>(enemy).unwrap().current,
            38,
            "火球应当在锁定的格子上炸到敌人（12 点伤害）"
        );
    }

    /// 远程火球：飞行时间比后摇长时，行动者必须**忙到落地**。
    ///
    /// 否则后摇一结束行动者就重新可决策，时间线立刻冻结虚拟时间——
    /// 飞行被截断，火球悬在半空、敌人一点血不掉。
    #[test]
    fn a_long_shot_keeps_the_shooter_busy_until_impact() {
        let mut app = test_app();
        let player = spawn_player(&mut app, Cell::new(0, 0), Vec3::new(1.0, 0.0, 1.0));
        // 6 格之外：飞 12 米要 1.5s，远超 0.50s 的后摇
        let enemy = spawn_enemy(&mut app, Cell::new(6, 0), Vec3::new(13.0, 0.0, 1.0));

        press(&mut app, KeyCode::KeyQ);
        // 12 帧 = 1.2s：后摇早在 0.8s 就过完了，此刻火球还在飞
        for _ in 0..12 {
            app.update();
        }
        let slot = slot_of(&app, player);
        assert!(
            matches!(slot, DecisionSlot::Recovery { .. }),
            "火球还在飞的时候射手不该拿到决策权（他还在飞行的后摇里），实际 {slot:?}"
        );

        for _ in 0..14 {
            app.update();
        }
        assert_eq!(
            app.world().get::<Health>(enemy).unwrap().current,
            38,
            "1.8s 之后火球应当已经落地并炸掉 12 点血"
        );
    }

    /// 火球打空地：落点范围内没有单位时完全落空，也不留残留实体。
    ///
    /// 玩家自己的火球不构成威胁（反应系统只看敌对来源），因此这一发不会被
    /// 「威胁冻结」半路截住——射手会一直忙到球落地。
    #[test]
    fn fireball_whiffs_on_empty_ground() {
        let mut app = test_app();
        spawn_player(&mut app, Cell::new(0, 0), Vec3::new(1.0, 0.0, 1.0));

        // 手边没有敌人：落点退回玩家自己的格 (0,0)，中心就在脚下
        press(&mut app, KeyCode::KeyQ);
        app.update();
        assert_eq!(
            actions::<FireballAction>(&mut app),
            1,
            "Q 应当先放出一发火球"
        );

        let mut frames = 0;
        while actions::<Fireball>(&mut app) > 0 && frames < 40 {
            app.update();
            frames += 1;
        }

        assert_eq!(
            actions::<Fireball>(&mut app),
            0,
            "火球到达落点后应当自行爆炸并销毁（跑完 {frames} 帧仍未销毁）"
        );
    }

    /// 撤销：行动实体销毁、决策槽立刻清空，花掉的钱由花钱的领域退回来
    /// （`ActionCancelled` → `combat::skills` 的退款 Observer）。
    ///
    /// 火球是「声明扣 2、撤销退 2 但收 2 的取消代价」→ 净额不变（3 点进 3 点出）。
    #[test]
    fn undo_refunds_the_declared_cost_and_frees_the_slot() {
        let mut app = test_app();
        let player = spawn_player(&mut app, Cell::new(0, 0), Vec3::new(1.0, 0.0, 1.0));
        spawn_enemy(&mut app, Cell::new(2, 0), Vec3::new(5.0, 0.0, 1.0));
        app.world_mut().get_mut::<Stamina>(player).unwrap().current = 3;

        press(&mut app, KeyCode::KeyQ);
        app.update();
        assert_eq!(
            app.world().get::<Stamina>(player).unwrap().current,
            1,
            "声明火球时先扣掉 2 点精力"
        );
        assert_eq!(slot_of(&app, player), DecisionSlot::Windup);

        app.world_mut().write_message(UndoCommand);
        app.update();

        assert_eq!(actions::<FireballAction>(&mut app), 0, "行动实体应当被销毁");
        assert_eq!(
            slot_of(&app, player),
            DecisionSlot::Empty,
            "撤销之后立刻能改主意"
        );
        assert_eq!(
            app.world().get::<Stamina>(player).unwrap().current,
            1,
            "退 2 收 2：撤销一次的净额不变"
        );
    }

    /// Focus：`Shift` + 决策键 = 花 1 点把这一手的前摇归零（下一帧就落地）。
    #[test]
    fn focus_spend_zeroes_the_windup_of_the_action() {
        let mut app = test_app();
        let player = spawn_player(&mut app, Cell::new(0, 0), Vec3::ZERO);

        press(&mut app, KeyCode::ShiftLeft);
        press(&mut app, KeyCode::ArrowUp);
        app.update();

        assert_eq!(
            app.world().resource::<Focus>().current,
            FOCUS_MAX - 1,
            "用掉 1 点 Focus"
        );
        let action = {
            let mut query = app.world_mut().query_filtered::<Entity, With<MoveAction>>();
            query.iter(app.world()).next().expect("应当声明出一条移动")
        };
        let schedule = *app.world().get::<ScheduledAction>(action).unwrap();
        assert_eq!(
            schedule.execute_at, schedule.declared_at,
            "前摇归零 = 执行时刻就是声明时刻"
        );

        // 下一帧就落地：速度出现、指针进入后摇（语义上它不属于前摇）
        //
        // 跑两帧是为了让时间线先把「玩家刚声明过、不必再等他」这件事落下去——
        // 声明那一帧时钟还停在"等输入"的状态里（暂停在帧末才生效 / 撤销）。
        app.update();
        app.update();
        assert_ne!(
            velocity_of(&mut app, player),
            Vec3::ZERO,
            "零前摇的行动下一帧就该动起来"
        );
        let slot = slot_of(&app, player);
        assert!(
            matches!(slot, DecisionSlot::Recovery { .. }),
            "零前摇的行动过了那一帧就该离开前摇，实际 {slot:?}"
        );
    }

    /// 没有 Focus 时退回普通前摇（不会扣成负数，也不会偷偷瞬发）。
    #[test]
    fn focus_is_not_spent_when_the_pool_is_empty() {
        let mut app = test_app();
        spawn_player(&mut app, Cell::new(0, 0), Vec3::ZERO);
        app.world_mut().resource_mut::<Focus>().current = 0;

        press(&mut app, KeyCode::ShiftLeft);
        press(&mut app, KeyCode::ArrowUp);
        app.update();

        assert_eq!(app.world().resource::<Focus>().current, 0);
        let mut query = app
            .world_mut()
            .query_filtered::<&ScheduledAction, With<MoveAction>>();
        let schedule = *query.iter(app.world()).next().expect("应当有移动行动");
        assert_eq!(
            schedule.windup(),
            timing::MOVE.windup,
            "没有余量就只能排前摇"
        );
    }

    /// 反应系统：敌对威胁瞄向玩家 → 世界冻结；玩家换一手 → 解冻继续打。
    #[test]
    fn a_threat_freezes_the_world_until_the_player_answers() {
        let mut app = test_app();
        let player = spawn_player(&mut app, Cell::new(0, 0), Vec3::ZERO);
        let enemy = spawn_enemy(&mut app, Cell::new(3, 0), Vec3::new(7.0, 0.0, 1.0));
        // 敌人正在前摇、且瞄着玩家脚下的格
        app.world_mut().spawn((
            ScheduledAction::declared_at(enemy, timing::SHOOT, 0.0),
            Threatens {
                cells: vec![Cell::new(0, 0)],
            },
        ));

        app.update();
        assert!(
            app.world().resource::<PauseReasons>().contains(THREAT),
            "威胁出现 → 世界冻住等玩家反应"
        );
        assert!(app.world().resource::<Time<Virtual>>().is_paused());

        // 玩家换一手：按下一个新意图（打断系统会先撤掉旧的未执行行动）
        press(&mut app, KeyCode::ArrowUp);
        for _ in 0..4 {
            app.update();
        }

        assert!(
            !app.world().resource::<PauseReasons>().contains(THREAT),
            "玩家表态之后威胁暂停应当撤销"
        );
        assert!(
            !app.world().resource::<Time<Virtual>>().is_paused(),
            "世界该继续跑：那一击该来就来"
        );
        assert_ne!(
            slot_of(&app, player),
            DecisionSlot::Empty,
            "玩家那一手应当已经声明出去"
        );
    }

    /// 打断：命中打向一个**正在前摇**的单位 → 那一手被打掉，决策槽立刻清空。
    #[test]
    fn a_landed_hit_interrupts_the_targets_windup() {
        let mut app = test_app();
        let enemy = spawn_enemy(&mut app, Cell::new(0, 0), Vec3::new(0.0, 0.0, 0.0));
        let action = app
            .world_mut()
            .spawn(ScheduledAction::declared_at(enemy, timing::SHOOT, 0.0))
            .id();
        // 玩家抡过来的横扫：力度 100 → 掷骰对抗必赢
        app.world_mut().spawn((
            Faction::Player,
            PhysicalDamage(15),
            InterruptPower(100),
            MeleeShape::default(),
            HitOnce::default(),
            Lifetime::default(),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ));

        app.update();

        assert!(
            app.world().get_entity(action).is_err(),
            "被打断的行动应当消失"
        );
        assert_eq!(
            slot_of(&app, enemy),
            DecisionSlot::Empty,
            "被打断的人立刻拿回决策槽"
        );
        assert_eq!(
            app.world().get::<Health>(enemy).unwrap().current,
            35,
            "打断与伤害同时发生（命中就是命中）"
        );
    }

    /// 打断只打**还没到点**的行动：已经到点的那一手谁也拦不住。
    #[test]
    fn an_action_that_came_due_is_not_interruptible() {
        let mut app = test_app();
        let target = spawn_enemy(&mut app, Cell::new(0, 0), Vec3::new(0.0, 0.0, 0.0));
        // 声明于 -1.0s 的行动：它在「现在」早就到点了
        let schedule = ScheduledAction::declared_at(target, timing::MOVE, -1.0);
        assert!(!schedule.pending(0.0), "这条行动应当已经到点");
        let action = app
            .world_mut()
            .spawn((schedule, MoveAction::default()))
            .id();
        let source = app.world_mut().spawn_empty().id();

        app.world_mut().trigger(InterruptEvent {
            entity: target,
            source,
            power: 100,
        });

        assert!(
            app.world().get_entity(action).is_ok(),
            "到点的行动打不断（它已经出去了）"
        );
    }

    /// 击杀链路：火球还在飞的时候杀了施法者——死亡走通用链路，
    /// 投射物照常落地结算，不留残留实体，也不 panic。
    #[test]
    fn a_fireball_still_lands_after_its_caster_dies() {
        let mut app = test_app();
        let player = spawn_player(&mut app, Cell::new(0, 0), Vec3::new(1.0, 0.0, 1.0));
        let enemy = spawn_enemy(&mut app, Cell::new(4, 0), Vec3::new(9.0, 0.0, 1.0));

        // 一条「立刻落地」的火球行动：射手这一帧就出手
        let schedule = ScheduledAction::declared_at(player, timing::SHOOT, 0.0).with_zero_windup();
        let _ = app.world_mut().spawn_scene(fireball_action_scene(
            Cell::new(0, 0),
            Cell::new(4, 0),
            schedule,
        ));
        app.world_mut()
            .entity_mut(player)
            .insert(DecisionSlot::Windup);
        app.update(); // 声明这一帧：零前摇的行动还不该落地
        app.update(); // 执行器发射投射物
        assert_eq!(actions::<Fireball>(&mut app), 1, "火球应当在飞行中");

        // 施法者被击杀：写一条足以致命的伤害，走通用死亡链路
        app.world_mut().write_message(DamageEvent {
            source: None,
            target: player,
            amount: 999,
        });
        app.update();
        assert!(
            app.world().get_entity(player).is_err(),
            "施法者应当死亡并被销毁"
        );

        // 场上没有玩家了 → 时间线不停表，火球照常飞完并结算
        for _ in 0..30 {
            app.update();
        }
        assert_eq!(actions::<Fireball>(&mut app), 0, "火球应当已经炸掉");
        assert_eq!(
            app.world().get::<Health>(enemy).unwrap().current,
            38,
            "施法者死了，飞出去的球照样落地（12 点伤害）"
        );
    }

    /// 阵亡走通用链路：血归零 → `DeathEvent` → 帧末销毁；尸体再挨打也不重复报丧。
    #[test]
    fn death_is_reported_once_and_the_entity_is_cleaned_up() {
        let mut app = test_app();
        let victim = spawn_enemy(&mut app, Cell::new(0, 0), Vec3::ZERO);

        for _ in 0..3 {
            app.world_mut().write_message(DamageEvent {
                source: None,
                target: victim,
                amount: 999,
            });
            app.update();
        }

        assert!(
            app.world().get_entity(victim).is_err(),
            "生命归零的实体应当被销毁"
        );
    }

    /// 整机装配冒烟：跨领域流水线（含帧末时钟）跑得起来，资源都在位。
    #[test]
    fn the_pipeline_runs_with_every_domain_installed() {
        let mut app = crate::test_support::headless_app();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            100,
        )));
        for _ in 0..5 {
            app.update();
        }
        assert!(
            app.world().get_resource::<PauseReasons>().is_some(),
            "时间线资源应当在整机里就位"
        );
        assert!(
            app.world().get_resource::<Focus>().is_some(),
            "反应资源 Focus 应当在整机里就位"
        );
    }

    /// 整机推进：玩家一直有活干时，敌人也必须真的在动。
    ///
    /// 这条测的是**没有死锁**：反应系统一旦写错（比如"玩家一表态就解冻"那条规则漏了），
    /// 世界会停在"双方都动不了"的状态里，而单测各自看都是绿的。
    #[test]
    fn the_world_keeps_making_progress() {
        let mut app = crate::test_support::headless_app();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            100,
        )));
        app.update(); // Startup：组装玩家 + 敌人

        let (player, enemy, start) = {
            let mut query = app.world_mut().query::<(Entity, &Faction, &Cell)>();
            let units: Vec<(Entity, Faction, Cell)> = query
                .iter(app.world())
                .map(|(entity, faction, cell)| (entity, *faction, *cell))
                .collect();
            let find = |wanted: Faction| {
                units
                    .iter()
                    .find(|(_, faction, _)| *faction == wanted)
                    .map(|(entity, _, cell)| (*entity, *cell))
                    .expect("应当有单位")
            };
            let (player, _) = find(Faction::Player);
            let (enemy, enemy_cell) = find(Faction::Enemy);
            (player, enemy, enemy_cell)
        };

        // 交替按两个方向键：方向变一次就声明一手，玩家因此长期处于"忙"的状态，
        // 世界（含敌人的 AI 与位移）就有时间流动
        for frame in 0..80 {
            if frame % 6 == 0 {
                let key = if (frame / 6) % 2 == 0 {
                    KeyCode::ArrowUp
                } else {
                    KeyCode::ArrowLeft
                };
                press(&mut app, key);
            }
            app.update();
        }

        let player_cell = app.world().get::<Cell>(player).copied();
        let enemy_cell = app.world().get::<Cell>(enemy).copied();
        assert_ne!(
            enemy_cell,
            Some(start),
            "玩家一直在动的时候，敌人应当走过来（否则就是死锁了）"
        );
        assert_ne!(player_cell, Some(Cell::new(1, 0)), "玩家也应当动过");
    }
}
