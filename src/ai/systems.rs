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

use crate::combat::defense::{ROLL_COST, Stamina, declare_roll, roll_step};
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

/// 执行意图：把 [`Intent`] 翻译成**行动实体**（AI 直接生成，不经玩家输入消息）。
///
/// 威胁预判已经在 [`decide_intent_system`] 里完成，这里只负责声明。
///
/// 「行动是统一实体」在两边是同一种东西：AI 直接调载荷工厂 + [`begin_action`]，
/// 玩家则由输入消息走各自的声明系统；**唯一的区别是触发源**。
#[allow(clippy::type_complexity)]
pub fn enemy_declare_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    mut timeline: ResMut<Timeline>,
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
            // 闪避直接生成 roll 行动：`RollCommand` 是**玩家输入消息**，AI 不该借用它
            // （借用会让玩家的 F 键把就绪的敌人也带着滚）
            Intent::Dodge => {
                let (dx, dz) = roll_step(
                    transform.translation,
                    *faction,
                    bodies
                        .iter()
                        .map(|(body, _, other)| (body.translation, *other)),
                );
                declare_roll(
                    &mut commands,
                    &mut timeline,
                    entity,
                    *cell,
                    Cell::new(cell.x + dx, cell.z + dz),
                    now,
                );
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
                declare_fireball_at(&mut commands, &mut timeline, entity, target_cell, now);
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

    /// 整机：AI 的闪避**直接生成自己的 roll 行动**，不经玩家输入消息，也不碰玩家的 `Ready`。
    ///
    /// 「行动是统一实体」的两种触发源在这里对齐：AI 走载荷工厂 + `begin_action`，
    /// 玩家走 `RollCommand`；区别只在触发源。曾经的实现是 AI 借用玩家的 `RollCommand`，
    /// 于是玩家按 F 会把就绪的敌人一起带着滚。
    #[test]
    fn a_dodging_enemy_declares_its_own_roll() {
        // 组装出来的敌人必须带 `Intent`，否则两个 AI 系统都匹配不到它（静默不行动）
        {
            let mut probe = crate::test_support::headless_app();
            probe.update();
            let mut intents = probe.world_mut().query_filtered::<&Intent, With<Faction>>();
            assert_eq!(
                intents.iter(probe.world()).count(),
                1,
                "敌人应当从组装开始就带 `Intent`，否则 AI 一行都不会执行"
            );
        }

        use crate::combat::{CollisionTarget, PhysicalDamage};
        use crate::movement::RollAction;

        let mut app = crate::test_support::headless_app();
        app.update(); // Startup：组装玩家 + 敌人

        let (player, enemy) = {
            let mut query = app.world_mut().query::<(Entity, &Faction)>();
            let units: Vec<(Entity, Faction)> = query
                .iter(app.world())
                .map(|(entity, faction)| (entity, *faction))
                .collect();
            let find = |wanted: Faction| {
                units
                    .iter()
                    .find(|(_, faction)| *faction == wanted)
                    .map(|(entity, _)| *entity)
                    .expect("应当有单位")
            };
            (find(Faction::Player), find(Faction::Enemy))
        };

        // 让敌人重新可决策：首帧 AI 可能已经替它声明了一个移动（本测试只关心"闪避由谁触发"）
        let stale: Vec<Entity> = app
            .world_mut()
            .query_filtered::<Entity, With<ScheduledAction>>()
            .iter(app.world())
            .collect();
        for action in stale {
            app.world_mut().entity_mut(action).despawn();
        }
        app.world_mut().entity_mut(enemy).insert(Ready);

        // 一发「正在前摇」的攻击瞄准敌人 → 意图变成 Dodge
        app.world_mut().spawn((
            Faction::Player,
            PhysicalDamage(10.0),
            CollisionTarget(enemy),
            ScheduledAction::declared_at(player, crate::timeline::timing::MELEE, 0.0),
        ));
        app.update();

        let rolls: Vec<Entity> = app
            .world_mut()
            .query_filtered::<Entity, With<RollAction>>()
            .iter(app.world())
            .collect();
        assert_eq!(rolls.len(), 1, "敌人应当自己声明一条翻滚");
        assert_eq!(
            app.world().get::<ScheduledAction>(rolls[0]).unwrap().actor,
            enemy,
            "行动必须挂在敌人自己身上"
        );
        assert!(
            app.world().get::<Ready>(player).is_some(),
            "玩家的 Ready 不该被 AI 的行动消耗掉"
        );
    }
}
