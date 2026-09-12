//! 两阶段结算：**阶段 1 只读裁决，阶段 2 统一落地**。
//!
//! 为什么分两阶段：同刻互击时，先手方不能因为「我先结算」而占便宜——
//! 裁决必须建立在**同一时刻的战场快照**上，之后再一起扣血。
//! 这样「互杀」在语义上成立：两边都能打出去，谁倒下由破势与顺序决定，而不是由系统调用顺序决定。
//!
//! ```text
//! phase1_arbitrate_system  只读：CollisionTarget → CombatResult（不碰任何状态）
//! phase2_apply_system      落地：DamageEvent / 反制 / 命中计数 / 清标记
//! ```
//!
//! 隔离缓冲用 [`Arbitration`]（`Resource`）而不是 `Local` 或消息：
//! `Local<T>` 是**每系统独享**的，两个阶段各自初始化会拿到两份缓冲、谁也读不到谁；
//! 而 `Event` + `Observer` 按项目约定只用于「即时、定向实体」的响应，这里不合适。

use bevy::prelude::*;

use crate::combat::attributes::{Armor, AttackFrame, AttackRange, Impact, PhysicalDamage};
use crate::combat::defense::{AttackResolved, DefenseOutcome, Dodging, Parrying};
use crate::combat::formula::DamageEvent;
use crate::combat::formula::domain::{
    AttackStats, DefenseState, HitOrder, Side, counter_damage, resolve_combat, resolve_defense,
};
use crate::combat::formula::physical_damage;
use crate::combat::formula::types::DamageType;
use crate::combat::lifecycle::{HitOnce, Projectile};
use crate::combat::targeting::CollisionTarget;

/// 阶段 1 → 阶段 2 的隔离缓冲。
///
/// 用 `Resource` 而不是 `Local`：两阶段是**两个系统**，各自初始化 `Local` 会导致
/// 缓冲不共享（阶段 1 写的那份没人读）。资源让「谁写谁读」在类型上一目了然。
#[derive(Resource, Debug, Default)]
pub struct Arbitration {
    results: Vec<CombatResult>,
    snapshot: Vec<(Entity, Entity, f32, AttackStats)>,
}

/// 一次攻击的裁决结果（阶段 1 的产物，阶段 2 的输入）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CombatResult {
    /// 攻击实体
    pub attack: Entity,
    /// 被打的目标
    pub target: Entity,
    /// 防御判定
    pub outcome: DefenseOutcome,
    /// 最终伤害（已扣护甲；被闪避 / 招架 / 打断时为 0）
    pub final_damage: f32,
    /// 被招架时攻击者应承受的反制伤害
    pub counter_damage: f32,
    /// 出手顺序（同刻互击时用于日志与复盘）
    pub order: HitOrder,
    /// 被破势打断的一方
    pub interrupted: Option<Side>,
}

/// 攻击实体的裁决参数查询（避免在系统签名里写超长元组）。
pub type AttackStatsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static PhysicalDamage,
        &'static CollisionTarget,
        Option<&'static AttackFrame>,
        Option<&'static AttackRange>,
        Option<&'static Impact>,
        Option<&'static Projectile>,
    ),
>;

/// 从查询行里装配纯数据（缺组件用 `Default`，因此任何攻击实体都能参与裁决）。
fn stats_from_row(
    frame: Option<&AttackFrame>,
    range: Option<&AttackRange>,
    impact: Option<&Impact>,
    damage: &PhysicalDamage,
) -> AttackStats {
    AttackStats::new(
        frame.map(|f| f.0).unwrap_or_default(),
        range.map(|r| r.world()).unwrap_or_default(),
        impact.map(|i| i.0).unwrap_or_default(),
        damage.0,
    )
}

/// **阶段 1**：只读裁决。
///
/// 两遍扫描：
/// 1. 把本帧所有攻击的**真实裁决参数**收集成快照（`(攻击实体, 目标, 真实距离) → AttackStats`）；
/// 2. 对每个攻击做防御判定 + 三层裁决。
///
/// 快照让「同刻互击」两边看到**同一份参数**，这正是两阶段结算的意义。
/// 参数全部只读 —— 本系统不修改任何组件。
pub fn phase1_arbitrate_system(
    attacks: AttackStatsQuery<'_, '_>,
    transforms: Query<&Transform>,
    dodging_q: Query<(), With<Dodging>>,
    parrying_q: Query<&Parrying>,
    armors: Query<&Armor>,
    mut buffer: ResMut<Arbitration>,
) {
    buffer.results.clear();
    buffer.snapshot.clear();
    let Arbitration { results, snapshot } = &mut *buffer;

    // 第一遍：真实参数快照
    for (attack, damage, collider, frame, range, impact, projectile) in &attacks {
        if projectile.is_some_and(|p| p.finished) {
            continue;
        }
        let distance = match (transforms.get(attack), transforms.get(collider.0)) {
            (Ok(from), Ok(to)) => from.translation.distance(to.translation),
            _ => 0.0,
        };
        snapshot.push((
            attack,
            collider.0,
            distance,
            stats_from_row(frame, range, impact, damage),
        ));
    }

    // 第二遍：防御判定 + 三层裁决
    for (attack, target, distance, stats) in snapshot.iter() {
        let parry = parrying_q.get(*target).ok();
        // 实体用 `to_bits()` 折成 `u64`：领域层因此不需要认识 Bevy 的 `Entity`
        let parry_target = parry.map(|p| p.target_attack.to_bits());
        let mut outcome = resolve_defense(
            DefenseState {
                dodging: dodging_q.get(*target).is_ok(),
                parrying: parry.is_some(),
            },
            attack.to_bits(),
            parry_target,
        );

        // 反击者 = 目标正在打回来的那次攻击（同刻互击）。
        // 只在快照里找，因此两遍都看到同一份数据。
        let riposte = snapshot
            .iter()
            .find(|(other, other_target, _, _)| *other == *target && *other_target == *attack)
            .map(|(_, _, _, other_stats)| *other_stats);

        let (hits, order, interrupted) = match riposte {
            Some(opponent) => {
                // 双方各按自己的参数裁决，距离取同一条真实距离。
                // `Side` 是**相对于裁决调用者**的视角：对本次攻击而言，
                // `Some(Side::Attacker)` 即「就是我自己被打断」。
                // 视角换算（+180°）由对方那一条另行完成，因此这里直接采用。
                let verdict = resolve_combat(stats, &opponent, *distance);
                (
                    verdict.attacker_hits,
                    verdict.order,
                    match verdict.interrupted {
                        Some(Side::Attacker) => Some(Side::Attacker),
                        Some(Side::Defender) => Some(Side::Defender),
                        None => None,
                    },
                )
            }
            // 单方攻击：目标获取已经保证够得着，这里只做防御判定
            None => (true, HitOrder::AttackerFirst, None),
        };

        let negated = outcome != DefenseOutcome::Landed || interrupted == Some(Side::Attacker);
        let armor = armors.get(*target).map(|a| a.0).unwrap_or(0.0);
        let final_damage = if negated || !hits {
            0.0
        } else {
            physical_damage(stats.damage, armor)
        };
        // 被破势打断时，防御结论改写为 `Interrupted`：免伤的原因不是「挡住了」
        if interrupted == Some(Side::Attacker) {
            outcome = DefenseOutcome::Interrupted;
        }

        results.push(CombatResult {
            attack: *attack,
            target: *target,
            outcome,
            final_damage,
            // 只有真正被招架才反制
            counter_damage: if outcome == DefenseOutcome::Parried {
                counter_damage(stats.damage)
            } else {
                0.0
            },
            order,
            interrupted,
        });
    }
}

/// **阶段 2**：统一落地。
///
/// 本阶段是**唯一**扣血路径：广播 [`DamageEvent`]（由 `request_damage_system` → `apply_damage` 落地），
/// 同时处理招架反制、投射物命中计数与临时标记清理。
pub fn phase2_apply_system(
    mut commands: Commands,
    mut buffer: ResMut<Arbitration>,
    mut resolved: MessageWriter<AttackResolved>,
    mut damage_events: MessageWriter<DamageEvent>,
    mut hit_once: Query<&mut HitOnce>,
    mut projectiles: Query<(&mut Projectile, Option<&mut crate::movement::Velocity>)>,
) {
    for result in buffer.results.drain(..) {
        resolved.write(AttackResolved {
            attacker: result.attack,
            target: result.target,
            outcome: result.outcome,
            counter: result.counter_damage,
        });

        if result.final_damage > 0.0 {
            damage_events.write(DamageEvent {
                target: result.target,
                amount: result.final_damage,
                kind: DamageType::Physical,
            });
        }
        // 招架反制：回敬伤害给攻击实体
        if result.outcome == DefenseOutcome::Parried && result.counter_damage > 0.0 {
            damage_events.write(DamageEvent {
                target: result.attack,
                amount: result.counter_damage,
                kind: DamageType::Physical,
            });
        }

        // 命中计数（射弹穿透 / 一次性攻击）
        if let Ok((mut projectile, velocity)) = projectiles.get_mut(result.attack) {
            projectile.current_hits += 1;
            if projectile.max_hits > 0 && projectile.current_hits >= projectile.max_hits {
                projectile.finished = true;
                if let Some(mut velocity) = velocity {
                    velocity.0 = Vec3::ZERO;
                }
            }
        }
        if let Ok(mut spent) = hit_once.get_mut(result.attack) {
            spent.spent = true;
        }

        // 临时标记用完就清
        commands.entity(result.attack).remove::<CollisionTarget>();
    }
}
