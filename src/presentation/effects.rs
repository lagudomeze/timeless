//! 命中特效：一次命中的地方冒一小簇**短命粒子**（会自己消失）。
//!
//! ## 分层
//!
//! 纯表现：**只读** [`DamageEvent`]，不改任何游戏状态，也不影响结算。
//! 因此它住在 `presentation`，与 HUD / 日志同级——`combat` 不需要知道"命中要好看"。
//!
//! ## 为什么不引粒子系统
//!
//! 这一版只有**一种**特效：命中处冒几个小方块、`EFFECT_SECONDS` 后消失。
//! 为此引 `bevy_hanabi` 之类的粒子库，会为一个用不上的特性付一整个依赖树的代价
//! （项目的规矩见 `docs/skills.md` 第七节：够用就不加机制）。
//! 这里用最朴素的做法：`spawn` 几个带 [`EffectParticle`] 的小方块，
//! 一个系统让它们**同时上浮 + 缩小**，到点自己 `despawn`。
//!
//! **触发条件**（满足任一条就该换成真正的粒子系统）：
//! ① 需要几十个以上粒子 / 复杂的发射形状；② 需要按材质做拖尾 / 光照；
//! ③ 需要多套特效资产（`.ron` 描述发射器）。届时本模块整体替换，外部不受影响
//! ——它对外只有"读 `DamageEvent`"这一个接口。

use bevy::prelude::*;

use crate::combat::{DamageEvent, Faction};

/// 特效存活时长（**虚拟秒**——它与世界一起冻结，理由见 `spawn_hit_effects_system`）。
pub const EFFECT_SECONDS: f32 = 0.35;
/// 每次命中冒几个粒子。
pub const PARTICLES_PER_HIT: usize = 5;
/// 粒子的边长（世界单位）。
pub const PARTICLE_SIZE: f32 = 0.14;
/// 粒子在一生中上浮的高度（世界单位）。
pub const PARTICLE_RISE: f32 = 0.7;

/// 命中粒子（自己会消失，不需要任何东西来清理它）。
///
/// 派生 `Reflect` + `reflect(Component)` 是为了**运行时能查它**（BRP 诊断锚点）：
/// 没注册的组件在远程协议里等于不存在——`world.query` 会**静默返回空**，
/// 看着像"特效没生成"，其实只是查不到（这个坑在排查这条特效时就踩了一次）。
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct EffectParticle {
    /// 已经活了多久（**虚拟秒**）
    pub age: f32,
    /// 出生位置（上浮的基准）
    pub origin: Vec3,
    /// 这一颗粒子的上浮 / 散开方向（每颗略有不同，看起来才不像一块板）
    pub drift: Vec3,
}

/// 颜色：谁挨打了就用**对方的阵营色**（玩家蓝 / 敌人红）——一眼看出打中了谁。
fn effect_color(struck: Faction) -> Color {
    match struck {
        Faction::Player => Color::srgb(0.45, 0.65, 1.0),
        Faction::Enemy => Color::srgb(1.0, 0.45, 0.40),
    }
}

/// 一次命中 → 在**被打中的那个单位**身上冒一小簇粒子。
///
/// 以 `DamageEvent.target` 的位置为准（不是攻击实体：射弹命中后就销毁了，
/// 而"挨打的地方"永远是目标身上）。
///
/// ⚠️ **用虚拟时间（`Time<Virtual>`）推进**，所以世界冻结时特效也停住——
/// 这点是刻意的：命中发生在结算那一瞬，冻结时特效定格在"刚打中"的样子，
/// 玩家解冻后接着看完。若改用 `Time<Real>`，等玩家思考时特效会自己播完消失，
/// 反而看不清刚才打到了谁。
pub fn spawn_hit_effects_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut damages: MessageReader<DamageEvent>,
    units: Query<(&Transform, &Faction)>,
) {
    for damage in damages.read() {
        let Ok((transform, faction)) = units.get(damage.target) else {
            continue; // 目标已经没了（同帧阵亡）：不加特效，也不 panic
        };
        let origin = transform.translation + Vec3::Y * PARTICLE_SIZE;
        let color = effect_color(*faction);
        for index in 0..PARTICLES_PER_HIT {
            // 用序号散开成一圈，避免所有粒子叠在同一个点上
            let angle = index as f32 / PARTICLES_PER_HIT as f32 * std::f32::consts::TAU;
            let drift = Vec3::new(angle.cos() * 0.25, 0.0, angle.sin() * 0.25);
            commands.spawn((
                EffectParticle {
                    age: 0.0,
                    origin,
                    drift,
                },
                Mesh3d(meshes.add(Cuboid::new(PARTICLE_SIZE, PARTICLE_SIZE, PARTICLE_SIZE))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: color,
                    unlit: true,
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                })),
                Transform::from_translation(origin),
            ));
        }
    }
}

/// 粒子动起来：上浮 + 缩小，到 [`EFFECT_SECONDS`] 自己销毁。
///
/// 「自己收尾」与项目里各执行器同一条纪律：没有人集中清理特效，
/// 每个粒子**自己**知道什么时候该消失。
pub fn animate_hit_effects_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    mut particles: Query<(Entity, &mut EffectParticle, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (entity, mut particle, mut transform) in &mut particles {
        particle.age += dt;
        if particle.age >= EFFECT_SECONDS {
            commands.entity(entity).despawn();
            continue;
        }
        // 0..1 的进度：位置从 origin 出发沿 drift 散开并上浮，缩放到 0
        let progress = particle.age / EFFECT_SECONDS;
        transform.translation =
            particle.origin + particle.drift * progress + Vec3::Y * PARTICLE_RISE * progress;
        transform.scale = Vec3::splat(1.0 - progress);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    /// 特效测试用的最小 App：只要特效系统 + 资产（粒子是现场造网格 / 材质）。
    fn effect_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            // 粒子是现场造网格 / 材质，所以 `Assets<..>` 得在位（`AssetPlugin` 提供它）
            .add_plugins(AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                50,
            )))
            .add_message::<DamageEvent>()
            .add_systems(
                Update,
                (spawn_hit_effects_system, animate_hit_effects_system).chain(),
            );
        app
    }

    fn particle_count(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<EffectParticle>>()
            .iter(app.world())
            .count()
    }

    /// 一次命中 → 冒 [`PARTICLES_PER_HIT`] 颗粒子；它们**自己会消失**。
    ///
    /// "自己会消失"是这条测试的重点：特效没有集中清理器，
    /// 若 `despawn` 那一步漏了，场上会慢慢攒下成千上万个死粒子。
    #[test]
    fn a_hit_spawns_particles_that_clean_themselves_up() {
        let mut app = effect_app();
        let target = app
            .world_mut()
            .spawn((Faction::Enemy, Transform::from_xyz(3.0, 0.0, 3.0)))
            .id();

        app.world_mut().write_message(DamageEvent {
            source: None,
            attacker: None,
            target,
            amount: 12,
            at: 0.0,
        });
        app.update();
        assert_eq!(
            particle_count(&mut app),
            PARTICLES_PER_HIT,
            "一次命中应当冒 {PARTICLES_PER_HIT} 颗粒子"
        );

        // 跑够 EFFECT_SECONDS（0.35s / 0.05s = 7 帧，多跑几帧保险）
        for _ in 0..12 {
            app.update();
        }
        assert_eq!(
            particle_count(&mut app),
            0,
            "粒子必须自己销毁，否则场上会攒下死特效"
        );
    }

    /// 粒子从**被打中的那个单位**身上冒出来，不是世界原点。
    #[test]
    fn particles_start_at_the_struck_unit() {
        let mut app = effect_app();
        let target = app
            .world_mut()
            .spawn((Faction::Player, Transform::from_xyz(5.0, 1.0, 2.0)))
            .id();
        app.world_mut().write_message(DamageEvent {
            source: None,
            attacker: None,
            target,
            amount: 1,
            at: 0.0,
        });
        app.update();

        let mut query = app.world_mut().query::<(&EffectParticle, &Transform)>();
        let origins: Vec<Vec3> = query
            .iter(app.world())
            .map(|(particle, transform)| {
                assert_eq!(
                    transform.translation, particle.origin,
                    "第一帧粒子应当还停在出生点上"
                );
                particle.origin
            })
            .collect();
        assert!(!origins.is_empty());
        for origin in origins {
            assert_eq!(
                (origin.x, origin.z),
                (5.0, 2.0),
                "粒子要落在被打中的单位脚下（不是原点、也不是别人）"
            );
            assert!(origin.y > 1.0, "粒子略高于脚底，免得埋进地面");
        }
    }

    /// 目标已经没了（同帧阵亡）时**不加特效、也不 panic**。
    #[test]
    fn a_hit_on_a_missing_target_is_skipped() {
        let mut app = effect_app();
        app.world_mut().write_message(DamageEvent {
            source: None,
            attacker: None,
            target: Entity::PLACEHOLDER,
            amount: 5,
            at: 0.0,
        });
        app.update();
        assert_eq!(particle_count(&mut app), 0, "目标不在就不该有特效");
    }

    /// 粒子会**动**（上浮 + 缩小），不是杵在原地。
    #[test]
    fn particles_rise_and_shrink_over_their_lifetime() {
        let mut app = effect_app();
        let target = app
            .world_mut()
            .spawn((Faction::Enemy, Transform::from_xyz(0.0, 0.0, 0.0)))
            .id();
        app.world_mut().write_message(DamageEvent {
            source: None,
            attacker: None,
            target,
            amount: 1,
            at: 0.0,
        });
        app.update();

        let sample = |app: &mut App| -> (Vec3, Vec3) {
            let mut query = app.world_mut().query::<(&EffectParticle, &Transform)>();
            query
                .iter(app.world())
                .next()
                .map(|(_, transform)| (transform.translation, transform.scale))
                .expect("应当还有粒子")
        };
        let (before_pos, before_scale) = sample(&mut app);

        app.update(); // 再走一帧（0.05s）
        let (after_pos, after_scale) = sample(&mut app);

        assert!(
            after_pos.y > before_pos.y,
            "粒子应当上浮：{before_pos:?} → {after_pos:?}"
        );
        assert!(
            after_scale.x < before_scale.x,
            "粒子应当缩小：{before_scale:?} → {after_scale:?}"
        );
    }
}
