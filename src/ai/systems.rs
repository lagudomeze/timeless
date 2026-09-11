//! 敌人 AI：**能决策就决策**。
//!
//! 无回合模型下敌人不等任何「轮」：只要有 [`Ready`]，就立刻按优先级选一个意图并
//! 声明行动实体；后摇结束恢复 `Ready` 后再次决策。
//! 动作的前摇 / 后摇本身（[`crate::timeline::timing`]）就是它的决策冷却。
//!
//! 决策顺序（**威胁优先于贪刀**）：
//!
//! 1. **威胁预判**：有正在前摇、且把我当目标的攻击 → `Dodge`
//! 2. 血少且贴脸 → `Retreat`（退开一格重新评估）
//! 3. 目标超出 `engage_range` → `Approach`
//! 4. 贴脸 → `Melee`
//! 5. 在武器射程内 → `Shoot`（火球锁住目标当前那一格，玩家可以先走开）
//! 6. 其余 → `Approach`
//!
//! 「选意图」（[`decide_intent_system`]）与「声明行动」（[`enemy_declare_system`]）
//! 拆成两个系统：HUD 因此能在敌人**动手之前**读到它想干什么。

use bevy::prelude::*;

use crate::combat::defense::{ROLL_COST, RollCommand, Stamina};
use crate::combat::skills::{declare_fireball_at, melee_action_scene};
use crate::combat::{AttackRange, Faction, Health};
use crate::movement::{Cell, move_action_scene, step_from_axis};
use crate::timeline::{CELL_SIZE, Ready, ScheduledAction, Timeline, begin_action};

use super::components::{EnemyBrain, Intent};

/// 贴脸判据（世界单位）：近战射程的 3/4 以内算「已经刻在脸上」。
const MELEE_REACH: f32 = CELL_SIZE * 0.75;

/// 战场上没有敌对目标时用的距离（等价于「够不着」）。
const NO_TARGET_DISTANCE: f32 = 10_000.0;

/// 一次决策的输入快照（系统间传递，因此不含引用）。
#[derive(Debug, Clone, Copy)]
struct Decision {
    distance: f32,
    range_world: f32,
    health_ratio: f32,
    engage_range: f32,
    cautious_ratio: f32,
    /// 是否有攻击正在前摇打向我
    in_danger: bool,
}

/// 选意图：只读规则，不生成任何实体。
///
/// 威胁预判在这里做（需要「谁瞄准了我」的完整信息），且**只看不写**——
/// 因此它是纯决策，声明与落地留给 [`enemy_declare_system`]。
#[allow(clippy::type_complexity)]
pub fn decide_intent_system(
    mut enemies: Query<
        (
            Entity,
            &Transform,
            &Faction,
            &Health,
            &AttackRange,
            &EnemyBrain,
            &mut Intent,
        ),
        With<Ready>,
    >,
    bodies: Query<(Entity, &Transform, &Faction, &Health)>,
    threats: Query<&crate::combat::targeting::CollisionTarget, With<ScheduledAction>>,
) {
    for (entity, transform, faction, health, range, brain, mut intent) in &mut enemies {
        let distance = bodies
            .iter()
            .filter(|(_, _, other, body_health)| other != &faction && body_health.is_alive())
            .map(|(_, body, _, _)| body.translation.distance(transform.translation))
            .min_by(f32::total_cmp)
            .unwrap_or(NO_TARGET_DISTANCE);

        let decision = Decision {
            distance,
            range_world: range.world(),
            health_ratio: if health.max > 0.0 {
                health.current / health.max
            } else {
                0.0
            },
            engage_range: brain.engage_range,
            cautious_ratio: brain.cautious_health_ratio,
            // 威胁 = 有攻击正在前摇、且把我当成目标
            in_danger: threats.iter().any(|collider| collider.0 == entity),
        };
        *intent = choose(&decision);
    }
}

/// 意图选择（纯函数：只读决策输入）。
fn choose(decision: &Decision) -> Intent {
    if decision.in_danger {
        return Intent::Dodge;
    }
    if decision.distance >= NO_TARGET_DISTANCE {
        return Intent::Idle;
    }
    if decision.health_ratio <= decision.cautious_ratio && decision.distance <= MELEE_REACH {
        return Intent::Retreat;
    }
    if decision.distance > decision.engage_range {
        return Intent::Approach;
    }
    if decision.distance <= MELEE_REACH {
        return Intent::Melee;
    }
    if decision.distance <= decision.range_world {
        return Intent::Shoot;
    }
    Intent::Approach
}

/// 执行意图：把 [`Intent`] 翻译成行动实体（防御意图则翻成 [`RollCommand`]）。
///
/// 威胁预判已经在 [`decide_intent_system`] 里完成，这里只负责声明。
#[allow(clippy::type_complexity)]
pub fn enemy_declare_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    mut timeline: ResMut<Timeline>,
    mut roll_requests: MessageWriter<RollCommand>,
    mut enemies: Query<(Entity, &Transform, &Cell, &Faction, &Stamina, &mut Intent), With<Ready>>,
    bodies: Query<(&Transform, &Cell, &Faction)>,
) {
    let now = now.elapsed_secs();

    for (entity, transform, cell, faction, stamina, mut intent) in &mut enemies {
        // 精力不够时不能真的闪：降级为普通决策结果
        if *intent == Intent::Dodge && !stamina.can_afford(ROLL_COST) {
            *intent = Intent::Approach;
        }

        // 目标位置：声明阶段只需要「朝谁走 / 打哪一格」
        let target = bodies
            .iter()
            .filter(|(_, _, other)| **other != *faction)
            .min_by(|(a, _, _), (b, _, _)| {
                a.translation
                    .distance_squared(transform.translation)
                    .total_cmp(&b.translation.distance_squared(transform.translation))
            })
            .map(|(body, body_cell, _)| (body.translation, *body_cell));

        match *intent {
            Intent::Idle => {}
            // 翻滚走玩家那条同一条路径（消息 → `declare_roll_system`），不重复实现
            Intent::Dodge => {
                roll_requests.write(RollCommand);
            }
            Intent::Approach | Intent::Retreat => {
                let Some((target_position, _)) = target else {
                    continue;
                };
                let mut to_target = target_position - transform.translation;
                if *intent == Intent::Retreat {
                    to_target = -to_target;
                }
                let axis = Vec2::new(to_target.x, to_target.z).normalize_or_zero();
                let (dx, dz) = step_from_axis(axis);
                if (dx, dz) == (0, 0) {
                    continue;
                }
                let to_cell = Cell::new(cell.x + dx, cell.z + dz);
                let draft = commands
                    .spawn_scene(move_action_scene(entity, *cell, to_cell, now))
                    .id();
                begin_action(&mut commands, &mut timeline, entity, draft);
            }
            Intent::Melee => {
                let draft = commands.spawn_scene(melee_action_scene(entity, now)).id();
                begin_action(&mut commands, &mut timeline, entity, draft);
            }
            Intent::Shoot => {
                let Some((_, target_cell)) = target else {
                    continue;
                };
                declare_fireball_at(
                    &mut commands,
                    &mut timeline,
                    entity,
                    transform.translation,
                    target_cell,
                    *faction,
                    now,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decision(distance: f32, range_world: f32, health_ratio: f32) -> Decision {
        Decision {
            distance,
            range_world,
            health_ratio,
            engage_range: 12.0,
            cautious_ratio: 0.35,
            in_danger: false,
        }
    }

    #[test]
    fn threat_comes_before_greed() {
        let mut d = decision(1.0, 2.0, 1.0);
        d.in_danger = true;
        assert_eq!(
            choose_with_threat(&d),
            Intent::Dodge,
            "有人正在打我时先闪，而不是贪一刀"
        );
    }

    #[test]
    fn no_target_means_idle() {
        assert_eq!(
            choose(&decision(NO_TARGET_DISTANCE, 4.0, 1.0)),
            Intent::Idle
        );
    }

    #[test]
    fn wounded_and_close_retreats() {
        assert_eq!(choose(&decision(1.0, 4.0, 0.2)), Intent::Retreat);
    }

    #[test]
    fn far_target_is_approached() {
        assert_eq!(choose(&decision(20.0, 4.0, 1.0)), Intent::Approach);
    }

    #[test]
    fn adjacent_target_is_meleed() {
        assert_eq!(choose(&decision(1.0, 4.0, 1.0)), Intent::Melee);
    }

    #[test]
    fn in_range_but_not_adjacent_fires() {
        assert_eq!(choose(&decision(3.0, 4.0, 1.0)), Intent::Shoot);
    }

    /// 与系统同一套判断，但把「威胁」也算进去（系统里 `in_danger` 由查询得出）。
    fn choose_with_threat(decision: &Decision) -> Intent {
        if decision.in_danger {
            return Intent::Dodge;
        }
        choose(decision)
    }
}
