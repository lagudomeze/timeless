//! 命中特效：一次命中的地方冒一小簇**短命粒子**（会自己消失）+ 一个**伤害数字**。
//!
//! ## 分层
//!
//! 纯表现：**只读** [`DamageEvent`]，不改任何游戏状态，也不影响结算。
//! 因此它住在 `presentation`，与 HUD / 日志同级——`combat` 不需要知道"命中要好看"。
//!
//! ## 为什么不引粒子系统
//!
//! 这一版只有**一种**特效：命中处冒几个小方块、[`EFFECT_SECONDS`] 后消失；
//! 伤害数字同理，`Text2d` + 一个系统让它上浮淡出。
//! 为此引 `bevy_hanabi` 之类的粒子库，会为一个用不上的特性付一整个依赖树的代价
//! （项目的规矩见 `docs/skills.md` 第七节：够用就不加机制）。
//! 这里用最朴素的做法：`spawn` 几个带 [`EffectParticle`] 的小方块 + 一个带
//! [`DamageNumber`] 的 `Text2d`，一个系统让它们**上浮 + 缩小 / 淡出**，
//! 到点自己 `despawn`。
//!
//! **触发条件**（满足任一条就该换成真正的粒子系统）：
//! ① 需要几十个以上粒子 / 复杂的发射形状；② 需要按材质做拖尾 / 光照；
//! ③ 需要多套特效资产（`.ron` 描述发射器）。届时本模块整体替换，外部不受影响
//! ——它对外只有"读 `DamageEvent`"这一个接口。
//!
//! ## 为什么两种反馈都要用**虚拟时间**
//!
//! 命中发生在结算那一瞬；世界这时可能正冻着等玩家表态。特效跟着虚拟时钟走，
//! 于是它定格在"刚打中"的样子，玩家解冻后接着看完。若用真实时间，
//! 等玩家思考的时候数字会自己飘走——恰恰在最需要它的时候消失。

use bevy::prelude::*;

use crate::combat::{DamageEvent, Faction};

/// 特效存活时长（**虚拟秒**——它与世界一起冻结，理由见 `spawn_hit_effects_system`）。
pub const EFFECT_SECONDS: f32 = 0.35;
/// 伤害数字的存活时长（虚拟秒）：比粒子长一点，数字要**读得完**。
pub const NUMBER_SECONDS: f32 = 0.8;
/// 伤害数字在一生中上浮的高度（世界单位）。
pub const NUMBER_RISE: f32 = 1.1;
/// 伤害数字的出生高度（世界单位，相对被打中单位的脚底）：别糊在纸片脸上。
pub const NUMBER_LIFT: f32 = 1.5;
/// 伤害数字的字号。
pub const NUMBER_FONT_SIZE: f32 = 28.0;
/// 每次命中冒几个粒子。
pub const PARTICLES_PER_HIT: usize = 5;
/// 粒子的边长（世界单位）。
pub const PARTICLE_SIZE: f32 = 0.14;
/// 粒子在一生中上浮的高度（世界单位）。
pub const PARTICLE_RISE: f32 = 0.7;

/// 伤害数字要用的字体（HUD 的那份；`setup_hud` 载入）。
///
/// **为什么做成资源**：伤害数字是**世界空间**的文字，它的系统在 `Update` 里
/// 按事件现场造实体——那时手里只有 `AssetServer`（异步、不该在这里 `.load()`）。
/// 把句柄放资源里，载入时机就归 Startup（与 HUD 同一份字体，不必再载一次）。
#[derive(Resource, Debug, Default, Clone)]
pub struct EffectFont(pub Handle<Font>);

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

/// 一次命中飘出的**伤害数字**（`-12`）。
///
/// 与 [`EffectParticle`] 同一套纪律：自己记年龄、自己上浮、到点自己销毁。
/// 它是**只读反馈**——不参与结算，也不被结算读取。
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct DamageNumber {
    /// 已经活了多久（**虚拟秒**）
    pub age: f32,
    /// 出生位置（上浮的基准）
    pub origin: Vec3,
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
    font: Res<EffectFont>,
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

        // 伤害数字：数字是**这一击的实际数值**（`amount` 已经算完护甲 / 格挡 / 招架），
        // 所以它读的是结算结果，而不是任何一处的"预计伤害"。
        let number_origin = transform.translation + Vec3::Y * NUMBER_LIFT;
        commands.spawn((
            DamageNumber {
                age: 0.0,
                origin: number_origin,
            },
            Text2d::new(format!("-{}", damage.amount)),
            TextFont::from_font_size(NUMBER_FONT_SIZE).with_font(font.0.clone()),
            TextColor(color),
            // 锚在**中心**：数字绕着出生点上浮，不会因为长短不同而左右偏
            bevy::sprite::Anchor::CENTER,
            Transform::from_translation(number_origin),
        ));
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

/// 伤害数字动起来：上浮 + 淡出，到 [`NUMBER_SECONDS`] 自己销毁。
///
/// 与粒子同一个"自己收尾"的纪律，但**用淡出而不是缩小**：数字要读得完，
/// 缩小会让最该看清的那一帧变得最小。同样走**虚拟时间**（理由见模块文档）。
pub fn animate_damage_numbers_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    mut numbers: Query<(Entity, &mut DamageNumber, &mut Transform, &mut TextColor)>,
) {
    let dt = time.delta_secs();
    for (entity, mut number, mut transform, mut color) in &mut numbers {
        number.age += dt;
        if number.age >= NUMBER_SECONDS {
            commands.entity(entity).despawn();
            continue;
        }
        let progress = number.age / NUMBER_SECONDS;
        transform.translation = number.origin + Vec3::Y * NUMBER_RISE * progress;
        // 前 60% 保持满不透明（读得清），之后才淡出
        let fade = ((1.0 - progress) / 0.4).clamp(0.0, 1.0);
        color.0.set_alpha(fade);
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
            .init_asset::<Font>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                50,
            )))
            // 伤害数字要用字体句柄；测试里给一个空句柄就够（字形不参与断言）
            .insert_resource(EffectFont(Handle::default()))
            .add_message::<DamageEvent>()
            .add_systems(
                Update,
                (
                    spawn_hit_effects_system,
                    animate_hit_effects_system,
                    animate_damage_numbers_system,
                )
                    .chain(),
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

    fn number_count(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<DamageNumber>>()
            .iter(app.world())
            .count()
    }

    /// **一次命中飘出一个伤害数字，数值就是这一击的结算值**（#60）。
    ///
    /// 读的是 `DamageEvent.amount`——它已经算完护甲 / 格挡 / 招架，
    /// 所以屏幕上显示的是**真的掉了多少血**，而不是任何一处的"预计伤害"。
    #[test]
    fn a_hit_floats_the_damage_it_dealt() {
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

        assert_eq!(number_count(&mut app), 1, "一次命中应当飘一个数字");
        let mut query = app.world_mut().query::<(&DamageNumber, &Text2d)>();
        let text = query
            .iter(app.world())
            .next()
            .map(|(_, text)| text.0.clone())
            .expect("数字实体应当在");
        assert_eq!(text, "-12", "数字要写这一击的**实际**数值");
    }

    /// 数字**上浮 + 淡出**，并到点自己消失（与粒子同一条"自己收尾"的纪律）。
    ///
    /// 用淡出而不是缩小：数字要读得完，缩小会让最该看清的那一帧最小。
    #[test]
    fn the_damage_number_rises_fades_and_cleans_itself_up() {
        let mut app = effect_app();
        let target = app
            .world_mut()
            .spawn((Faction::Player, Transform::from_xyz(0.0, 0.0, 0.0)))
            .id();
        app.world_mut().write_message(DamageEvent {
            source: None,
            attacker: None,
            target,
            amount: 7,
            at: 0.0,
        });
        app.update();

        let sample = |app: &mut App| -> (Vec3, f32) {
            let mut query = app
                .world_mut()
                .query::<(&DamageNumber, &Transform, &TextColor)>();
            query
                .iter(app.world())
                .next()
                .map(|(_, transform, color)| (transform.translation, color.0.alpha()))
                .expect("数字应当还在")
        };
        let (before_pos, before_alpha) = sample(&mut app);
        assert_eq!(before_alpha, 1.0, "刚冒出来时应当完全不透明（看得清）");

        // 跑到后半段（0.8s 里的 0.5s：10 帧）
        for _ in 0..10 {
            app.update();
        }
        let (after_pos, after_alpha) = sample(&mut app);
        assert!(
            after_pos.y > before_pos.y,
            "数字应当上浮：{before_pos:?} → {after_pos:?}"
        );
        assert!(
            after_alpha < before_alpha,
            "数字应当淡出：{before_alpha} → {after_alpha}"
        );

        // 跑够 NUMBER_SECONDS 之后自己消失
        for _ in 0..12 {
            app.update();
        }
        assert_eq!(number_count(&mut app), 0, "数字必须自己销毁");
    }

    /// 目标同帧阵亡时**不许 panic**，也不该留下没有主人的数字。
    #[test]
    fn a_hit_on_a_gone_target_leaves_no_number() {
        let mut app = effect_app();
        let target = app
            .world_mut()
            .spawn((Faction::Enemy, Transform::default()))
            .id();
        app.world_mut().entity_mut(target).despawn();

        app.world_mut().write_message(DamageEvent {
            source: None,
            attacker: None,
            target,
            amount: 5,
            at: 0.0,
        });
        app.update();
        assert_eq!(number_count(&mut app), 0, "目标没了就不该冒数字");
        assert_eq!(particle_count(&mut app), 0, "也不该冒粒子");
    }
}
