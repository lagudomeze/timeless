//! # 战斗领域：属性组件 + 动作载荷 + 两阶段结算
//!
//! 行动即实体：`Attack` / `Parry` / `Fireball` 是动作实体的载荷组件，
//! 由时间线调度（`Declared → Pending → Committed`，见 timeline.rs），
//! 本模块只负责「Committed 之后的裁决与应用」：
//!
//! ```text
//! combat_phase1（计算，只读 + 挂载 CombatResult）
//!   → 闪避/招架/射程/同刻互击（帧→距离→破势）裁决
//! combat_phase2（应用，统一扣血 + despawn）
//! ```
//!
//! 防御机制：`Roll` 执行后挂 `Dodging`、`Parry` 执行后挂 `Parrying`（见 movement.rs），
//! 阶段 1 只查标记，不做跨实体耦合。

use std::collections::VecDeque;

use bevy::ecs::template::FromTemplate;
use bevy::prelude::*;

use timeless_domain::combat::{
    AttackStats as DomainAttackStats, HitOrder, Side, resolve_attack, resolve_combat,
};

use crate::display::map::{cell_x, cell_z};
use crate::menu::MenuSelection;
use crate::movement::{Destination, GridMath, LinearVelocity, MoveTo, Position, Projectile, Roll};
use crate::timeline::{
    Committed, Declared, Pending, ScheduledAction, TICK_MS, TimeLineState, TurnPhase,
};

// ─────────────────────────── 属性组件（单位常驻） ───────────────────────────

/// 生命值
#[derive(Component, Debug, Clone, Copy)]
pub struct Health {
    pub current: u32,
    pub max: u32,
}

impl Health {
    pub fn new(max: u32) -> Self {
        Self { current: max, max }
    }

    pub fn is_alive(&self) -> bool {
        self.current > 0
    }
}

/// 单次攻击伤害（与 `Health` 对应的输出侧组件；声明动作时快照进载荷）
#[derive(Component, Debug, Clone, Copy)]
pub struct Damage(pub u32);

/// 攻击帧：数值越小，出招越快（换算成前摇 `cast_duration = frame * TICK_MS`）
#[derive(Component, Debug, Clone, Copy)]
pub struct AttackFrame(pub u32);

/// 攻击射程（网格距离，按切比雪夫距离计）
#[derive(Component, Debug, Clone, Copy)]
pub struct AttackRange(pub u32);

/// 破势：双方同刻互击时，破势高者打断对方攻击
#[derive(Component, Debug, Clone, Copy)]
pub struct Impact(pub u32);

/// 精力：翻滚取消 / 招架 / 火球消耗
#[derive(Component, Debug, Clone, Copy)]
pub struct Stamina {
    pub current: u32,
    pub max: u32,
}

impl Stamina {
    pub fn new(max: u32) -> Self {
        Self { current: max, max }
    }

    pub fn try_spend(&mut self, amount: u32) -> bool {
        if self.current >= amount {
            self.current -= amount;
            true
        } else {
            false
        }
    }
}

/// 玩家 / 敌人标记（查询过滤与规则归属用）
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Player;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Enemy;

// ─────────────────────────── 动作载荷（动作实体） ───────────────────────────

/// 攻击动作：声明时快照目标与数值；执行时按 射程 / 同刻破势 裁决
#[derive(Component, Debug, Clone, Copy, FromTemplate)]
pub struct Attack {
    pub target: Entity,
    pub damage: u32,
    pub range: u32,
    pub impact: u32,
}

/// 招架动作（反应生成）：绑定到具体的攻击实体
#[derive(Component, Debug, Clone, Copy, FromTemplate)]
pub struct Parry {
    pub target_attack: Entity,
}

/// 火球动作：提交时锁定目标格，结算时生成投射物
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Fireball {
    pub target: IVec2,
    pub speed: f32,
    pub amount: u32,
    pub radius: u32,
}

/// 爆炸伤害（投射物载荷）：到达目的地后对半径内所有 `Health` 实体结算
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct ExplosionDamage {
    pub amount: u32,
    pub radius: u32,
}

/// 火球投射物共享渲染资源（setup 构建一次，施放时复用）
#[derive(Resource, Clone)]
pub struct FireballAssets {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

// ─────────────────────────── 防御标记与结算结果 ───────────────────────────

/// 闪避标记（挂在单位上）：本回合翻滚中，攻击对其落空
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dodging;

/// 招架标记（挂在单位上）：绑定到被招架的攻击实体
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parrying {
    pub target_attack: Entity,
}

/// 结算结果（阶段 1 计算挂载，阶段 2 应用）：临时组件，动作实体销毁时一并移除
#[derive(Component, Debug, Clone, Copy)]
pub struct CombatResult {
    pub target: Entity,
    pub final_damage: u32,
    pub counter_damage: u32,
    pub order: HitOrder,
}

/// 战斗日志（屏幕 UI 用）：环形保留最近 N 条消息，格式与控制台一致
#[derive(Resource, Debug)]
pub struct BattleLog {
    pub entries: VecDeque<String>,
    pub max: usize,
}

impl Default for BattleLog {
    fn default() -> Self {
        Self {
            entries: VecDeque::with_capacity(8),
            max: 8,
        }
    }
}

impl BattleLog {
    pub fn push(&mut self, msg: String) {
        if self.entries.len() >= self.max {
            self.entries.pop_front();
        }
        self.entries.push_back(msg);
    }
}

// ─────────────────────────── 消息 ───────────────────────────

/// 一次命中（伤害已应用，供日志 / 后续特效订阅）
#[derive(Message, Debug, Clone, Copy)]
pub struct HitLanded {
    pub attacker: Entity,
    pub defender: Entity,
    pub damage: u32,
    pub order: HitOrder,
}

/// 投射物到达目的地（`movement::projectile_motion_system` 发出，本文件
/// `explosion_system` 消费）：飞行只管运动，落地后果由战斗领域裁决。
#[derive(Message, Debug, Clone, Copy)]
pub struct ProjectileArrived {
    pub source: Entity,
    pub cell: IVec2,
}

// ─────────────────────────── 查询别名 ───────────────────────────

type PositionQuery<'w, 's> = Query<'w, 's, &'static Position>;
type DodgingQuery<'w, 's> = Query<'w, 's, (), With<Dodging>>;
type ParryingQuery<'w, 's> = Query<'w, 's, &'static Parrying>;

/// 敌人 AI 查询（位置 + 攻击属性）
type EnemyAiQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        &'static AttackFrame,
        &'static AttackRange,
        &'static Impact,
        &'static Damage,
    ),
    With<Enemy>,
>;

/// 玩家对位查询（实体 + 位置）
type PlayerPairQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Position), (With<Player>, Without<Enemy>)>;

/// 动作标签查询（HUD / 调试面板共用）：读某执行者当前 Declared / Pending 动作
pub(crate) type ActionLabelQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ScheduledAction,
        Option<&'static Attack>,
        Option<&'static MoveTo>,
        Option<&'static Roll>,
        Option<&'static Fireball>,
        Option<&'static Parry>,
    ),
    Or<(With<Declared>, With<Pending>)>,
>;

// ─────────────────────────── 声明 / 调度侧系统 ───────────────────────────

/// 敌人 AI：决策阶段开始即声明动作（进入射程锁定攻击，否则逼近一格）。
/// 动作 = 实体，经时间线调度执行；仅在本回合尚未声明时执行，避免重复决策。
pub fn ai_system(
    tl: Res<TimeLineState>,
    mut commands: Commands,
    enemy_q: EnemyAiQuery<'_, '_>,
    player_q: PlayerPairQuery<'_, '_>,
    declared: Query<&ScheduledAction, With<Declared>>,
) {
    if tl.phase != TurnPhase::Decision {
        return;
    }
    let Ok((enemy, pos, frame, range, impact, damage)) = enemy_q.single() else {
        return;
    };
    if declared.iter().any(|s| s.actor == enemy) {
        return; // 本回合已声明
    }
    let Ok((player, player_pos)) = player_q.single() else {
        return;
    };

    if pos.0.chebyshev(player_pos.0) <= range.0 {
        commands.spawn_scene(bsn! {
            Attack {
                target: {player},
                damage: {damage.0},
                range: {range.0},
                impact: {impact.0},
            }
            ScheduledAction {
                execute_at: 0,
                cast_duration: {frame.0 as u64 * TICK_MS},
                actor: {enemy},
            }
            Declared
        });
        info!("[AI] 敌人锁定攻击（帧 {}）", frame.0);
    } else {
        let velocity = (player_pos.0 - pos.0).signum();
        commands.spawn_scene(bsn! {
            MoveTo { velocity: {velocity} }
            ScheduledAction { execute_at: 0, cast_duration: 0, actor: {enemy} }
            Declared
        });
        info!("[AI] 敌人逼近玩家，速度 {velocity:?}");
    }
}

/// 威胁检测：双方都有 Pending 攻击 → 进入 Reaction 暂停等待玩家反应
pub fn reaction_trigger_system(
    mut tl: ResMut<TimeLineState>,
    mut menu: ResMut<MenuSelection>,
    player_q: Query<Entity, With<Player>>,
    enemy_q: Query<Entity, With<Enemy>>,
    actions: Query<&ScheduledAction, (With<Attack>, With<Pending>)>,
) {
    if tl.phase != TurnPhase::Resolving || tl.reaction_resolved {
        return;
    }
    let (Ok(player), Ok(enemy)) = (player_q.single(), enemy_q.single()) else {
        return;
    };
    let player_attacks = actions.iter().any(|s| s.actor == player);
    let enemy_attacks = actions.iter().any(|s| s.actor == enemy);
    if player_attacks && enemy_attacks {
        tl.phase = TurnPhase::Reaction;
        menu.index = 0; // 反应列表从头开始
        info!("[威胁] 双方都将攻击 —— 进入反应阶段（暂停等待选择）");
    }
}

// ─────────────────────────── 两阶段结算 ───────────────────────────

/// 防御覆盖：闪避 → 招架。命中被防御时返回 Some(结果)，否则 None（照常裁决）。
fn defense_outcome(
    defender: Entity,
    attack_entity: Entity,
    damage: u32,
    dodging: &DodgingQuery,
    parrying: &ParryingQuery,
    log: &mut BattleLog,
) -> Option<CombatResult> {
    if dodging.get(defender).is_ok() {
        info!("[闪避] 攻击被闪避！");
        log.push("[闪避] 攻击被闪避！".to_string());
        return Some(CombatResult {
            target: defender,
            final_damage: 0,
            counter_damage: 0,
            order: HitOrder::Simultaneous,
        });
    }
    if let Ok(parry) = parrying.get(defender)
        && parry.target_attack == attack_entity
    {
        let counter = damage.div_ceil(2);
        info!("[招架] 格挡成功，反制 {counter} 伤害！");
        log.push(format!("[招架] 格挡成功，反制 {counter} 伤害！"));
        return Some(CombatResult {
            target: defender,
            final_damage: 0,
            counter_damage: counter,
            order: HitOrder::Simultaneous,
        });
    }
    None
}

/// 组装单次攻击的结算结果（命中判定已由调用方给出）
#[allow(clippy::too_many_arguments)]
fn outcome_for(
    attack_entity: Entity,
    attack: &Attack,
    hits: bool,
    interrupted: bool,
    order: HitOrder,
    dodging: &DodgingQuery,
    parrying: &ParryingQuery,
    log: &mut BattleLog,
) -> CombatResult {
    if hits
        && let Some(def) = defense_outcome(
            attack.target,
            attack_entity,
            attack.damage,
            dodging,
            parrying,
            log,
        )
    {
        return def;
    }
    CombatResult {
        target: attack.target,
        final_damage: if hits && !interrupted {
            attack.damage
        } else {
            0
        },
        counter_damage: 0,
        order,
    }
}

/// 招架执行器：`Committed` 招架动作 → 在防御者身上挂 `Parrying`
/// （绑定到被招架的攻击实体，阶段 1 据此判定格挡 + 反制）。
pub fn parry_executor(
    mut commands: Commands,
    q: Query<(Entity, &ScheduledAction, &Parry), With<Committed>>,
) {
    for (action, scheduled, parry) in &q {
        commands.entity(scheduled.actor).insert(Parrying {
            target_attack: parry.target_attack,
        });
        commands.entity(action).despawn();
    }
}

/// 结算阶段 1（计算）：所有 `Committed` 攻击批量裁决，挂载 `CombatResult`，
/// 不修改任何 HP。同刻互击用领域层 `resolve_combat`（帧相同 → 距离 → 破势）。
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn combat_phase1_system(
    mut commands: Commands,
    attacks: Query<(Entity, &ScheduledAction, &Attack), With<Committed>>,
    positions: PositionQuery<'_, '_>,
    dodging_q: DodgingQuery<'_, '_>,
    parrying_q: ParryingQuery<'_, '_>,
    player_q: Query<Entity, With<Player>>,
    enemy_q: Query<Entity, With<Enemy>>,
    mut log: ResMut<BattleLog>,
) {
    let attacks: Vec<(Entity, ScheduledAction, Attack)> = attacks
        .iter()
        .map(|(entity, scheduled, attack)| (entity, *scheduled, *attack))
        .collect();
    let player = player_q.single().ok();
    let enemy = enemy_q.single().ok();
    let who = |entity: Entity| -> &'static str {
        if Some(entity) == player {
            "玩家"
        } else if Some(entity) == enemy {
            "敌人"
        } else {
            "单位"
        }
    };
    let mut processed: Vec<Entity> = Vec::new();

    for (entity, scheduled, attack) in &attacks {
        if processed.contains(entity) {
            continue;
        }
        // 同刻互击：另一 Committed 攻击，execute_at 相同且互为目标
        let opponent = attacks.iter().find(|(oe, os, oa)| {
            os.execute_at == scheduled.execute_at
                && oe != entity
                && oa.target == scheduled.actor
                && attack.target == os.actor
        });
        if let Some((op_entity, op_scheduled, op_attack)) = opponent {
            let dist = pos_dist(*scheduled, *attack, &positions);
            let a_stats = DomainAttackStats::new(0, attack.range, attack.impact, attack.damage);
            let o_stats =
                DomainAttackStats::new(0, op_attack.range, op_attack.impact, op_attack.damage);
            let r = resolve_combat(&a_stats, &o_stats, dist);
            let order_str = match r.order {
                HitOrder::AttackerFirst => format!("{}先手", who(scheduled.actor)),
                HitOrder::DefenderFirst => format!("{}先手", who(op_scheduled.actor)),
                HitOrder::Simultaneous => "同时命中".to_string(),
            };
            info!("[裁决] {order_str}（距离 {dist}）");
            log.push(format!("[裁决] {order_str}（距离 {dist}）"));
            if let Some(side) = r.interrupted {
                let who_str = match side {
                    Side::Attacker => who(scheduled.actor),
                    Side::Defender => who(op_scheduled.actor),
                };
                info!("[破势] {who_str} 的攻击被打断！");
                log.push(format!("[破势] {who_str} 的攻击被打断！"));
            }
            let result_a = outcome_for(
                *entity,
                attack,
                r.attacker_hits,
                r.interrupted == Some(Side::Attacker),
                r.order,
                &dodging_q,
                &parrying_q,
                &mut log,
            );
            let result_b = outcome_for(
                *op_entity,
                op_attack,
                r.defender_hits,
                r.interrupted == Some(Side::Defender),
                r.order,
                &dodging_q,
                &parrying_q,
                &mut log,
            );
            commands.entity(*entity).insert(result_a);
            commands.entity(*op_entity).insert(result_b);
            processed.push(*op_entity);
            continue;
        }

        // 单方攻击：射程裁决（命中后才考虑闪避 / 招架）
        let dist = pos_dist(*scheduled, *attack, &positions);
        let stats = DomainAttackStats::new(0, attack.range, attack.impact, attack.damage);
        let hits = resolve_attack(&stats, dist);
        if !hits {
            info!(
                "[裁决] {} 攻击落空（距离 {dist} 超出射程 {}）",
                who(scheduled.actor),
                attack.range
            );
            log.push(format!(
                "[裁决] {} 攻击落空（距离 {dist} 超出射程 {}）",
                who(scheduled.actor),
                attack.range
            ));
        }
        let result = outcome_for(
            *entity,
            attack,
            hits,
            false,
            HitOrder::AttackerFirst,
            &dodging_q,
            &parrying_q,
            &mut log,
        );
        commands.entity(*entity).insert(result);
    }
}

/// 结算阶段 2（应用）：统一扣除 HP、应用招架反制，随后销毁动作实体。
pub fn combat_phase2_system(
    mut commands: Commands,
    results: Query<(Entity, &CombatResult, &ScheduledAction), With<Committed>>,
    mut health_q: Query<&mut Health>,
    mut ev_hit: MessageWriter<HitLanded>,
) {
    for (action, result, scheduled) in &results {
        // 招架反制：原攻击方吃一半伤害
        if result.counter_damage > 0
            && let Ok(mut hp) = health_q.get_mut(scheduled.actor)
        {
            apply_hit(
                &mut hp,
                result.target,
                scheduled.actor,
                result.counter_damage,
                HitOrder::Simultaneous,
                &mut ev_hit,
            );
        }
        // 主伤害
        if result.final_damage > 0
            && let Ok(mut hp) = health_q.get_mut(result.target)
        {
            apply_hit(
                &mut hp,
                scheduled.actor,
                result.target,
                result.final_damage,
                result.order,
                &mut ev_hit,
            );
        }
        commands.entity(action).despawn();
    }
}

/// 火球执行器：`Committed` 火球动作 → 生成投射物（初始位置 = 释放者 + 朝向目标一格）
pub fn fireball_executor(
    mut commands: Commands,
    assets: Res<FireballAssets>,
    q: Query<(Entity, &ScheduledAction, &Fireball), With<Committed>>,
    positions: PositionQuery<'_, '_>,
    mut log: ResMut<BattleLog>,
) {
    for (action, scheduled, fb) in &q {
        let Some(from) = positions.get(scheduled.actor).ok().map(|p| p.0) else {
            commands.entity(action).despawn();
            continue;
        };
        spawn_fireball(
            &mut commands,
            from,
            fb.target,
            fb.speed,
            fb.amount,
            fb.radius,
            assets.mesh.clone(),
            assets.material.clone(),
        );
        info!("[技能] 玩家施放火球 → ({},{})", fb.target.x, fb.target.y);
        log.push(format!(
            "[技能] 玩家施放火球 → ({},{})",
            fb.target.x, fb.target.y
        ));
        commands.entity(action).despawn();
    }
}

/// 爆炸结算（消费 `ProjectileArrived`）：到达目标后，范围内有 PC/NPC 才产生伤害，
/// 否则落空（敌人已移动可躲避）。
pub fn explosion_system(
    mut arrived: MessageReader<ProjectileArrived>,
    mut commands: Commands,
    mut log: ResMut<BattleLog>,
    boom_q: Query<&ExplosionDamage>,
    mut targets: Query<(Entity, &Position, &mut Health)>,
    mut ev_hit: MessageWriter<HitLanded>,
) {
    for m in arrived.read() {
        let Ok(boom) = boom_q.get(m.source) else {
            continue; // 非爆炸类投射物（防御性跳过）
        };
        let mut hit_count = 0;
        for (target, pos, mut hp) in &mut targets {
            if target == m.source {
                continue;
            }
            if pos.0.chebyshev(m.cell) <= boom.radius {
                apply_hit(
                    &mut hp,
                    m.source,
                    target,
                    boom.amount,
                    HitOrder::Simultaneous,
                    &mut ev_hit,
                );
                hit_count += 1;
            }
        }
        if hit_count > 0 {
            info!(
                "[爆炸] 火球命中 ({},{})：{} 伤害 / 半径 {}，命中 {hit_count} 个单位",
                m.cell.x, m.cell.y, boom.amount, boom.radius
            );
            log.push(format!(
                "[爆炸] 火球命中 ({},{})：{} 伤害 / 半径 {}，命中 {hit_count} 个单位",
                m.cell.x, m.cell.y, boom.amount, boom.radius
            ));
        } else {
            info!(
                "[爆炸] 火球落空（目标格 ({},{}) 无单位）",
                m.cell.x, m.cell.y
            );
            log.push(format!(
                "[爆炸] 火球落空（目标格 ({},{}) 无单位）",
                m.cell.x, m.cell.y
            ));
        }
        commands.entity(m.source).despawn();
    }
}

/// 死亡检查 + 阶段推进：血量归零的实体统一清场；任一方阵亡 → GameOver。
/// 每帧运行，火球在任意时刻击杀也能正确收尾。
pub fn death_check_system(
    mut tl: ResMut<TimeLineState>,
    mut commands: Commands,
    mut log: ResMut<BattleLog>,
    q: Query<(Entity, &Health)>,
    player_q: Query<&Health, (With<Player>, Without<Enemy>)>,
    enemy_q: Query<&Health, (With<Enemy>, Without<Player>)>,
) {
    for (entity, hp) in &q {
        if !hp.is_alive() {
            commands.entity(entity).despawn();
            info!("☠ 实体 {:?} 被击败！（回合 {}）", entity, tl.global_tick);
        }
    }
    if tl.phase == TurnPhase::GameOver {
        return;
    }
    let p_alive = player_q.single().is_ok_and(Health::is_alive);
    let e_alive = enemy_q.single().is_ok_and(Health::is_alive);
    if !p_alive || !e_alive {
        tl.phase = TurnPhase::GameOver;
        log.push("── 战斗结束 ──".to_string());
        info!("══ 战斗结束（回合 {}）══", tl.global_tick);
    }
}

// ─────────────────────────── 工具 ───────────────────────────

/// 应用伤害并广播 HitLanded（近战命中与投射物爆炸共用同一入口）
pub(crate) fn apply_hit(
    hp: &mut Health,
    attacker: Entity,
    defender: Entity,
    damage: u32,
    order: HitOrder,
    ev: &mut MessageWriter<HitLanded>,
) {
    hp.current = hp.current.saturating_sub(damage);
    ev.write(HitLanded {
        attacker,
        defender,
        damage,
        order,
    });
}

/// 动作载荷 → 简短中文描述（HUD / 调试面板共用）
pub(crate) fn intent_label(
    attack: Option<&Attack>,
    mov: Option<&MoveTo>,
    roll: Option<&Roll>,
    fireball: Option<&Fireball>,
    parry: Option<&Parry>,
) -> String {
    if parry.is_some() {
        "招架".to_string()
    } else if fireball.is_some() {
        "火球".to_string()
    } else if roll.is_some() {
        "翻滚".to_string()
    } else if mov.is_some() {
        "移动".to_string()
    } else if attack.is_some() {
        "攻击".to_string()
    } else {
        "待机".to_string()
    }
}

/// 查询某执行者当前声明（Declared / Pending）的动作标签
pub(crate) fn action_label(actor: Entity, actions: &ActionLabelQuery) -> String {
    actions
        .iter()
        .find(|(scheduled, ..)| scheduled.actor == actor)
        .map(|(_, attack, mov, roll, fireball, parry)| {
            intent_label(attack, mov, roll, fireball, parry)
        })
        .unwrap_or_else(|| "待机".to_string())
}

/// 两个动作实体之间的网格距离（任意一方位置缺失时按最大距离处理）
fn pos_dist(scheduled: ScheduledAction, attack: Attack, positions: &PositionQuery) -> u32 {
    match (positions.get(scheduled.actor), positions.get(attack.target)) {
        (Ok(a), Ok(b)) => a.0.chebyshev(b.0),
        _ => u32::MAX,
    }
}

/// 生成火球投射物：实体 = `Transform`（初始位置）+ `LinearVelocity`（速度）+
/// `Destination` + `Projectile` + `ExplosionDamage` + 渲染组件。
/// 初始位置 = 释放者 + 朝向目标一格。
#[allow(clippy::too_many_arguments)]
pub fn spawn_fireball(
    commands: &mut Commands,
    from: IVec2,
    to: IVec2,
    speed: f32,
    amount: u32,
    radius: u32,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
) {
    let spawn = from + (to - from).signum();
    let from_w = Vec2::new(cell_x(spawn.x), cell_z(spawn.y));
    let to_w = Vec2::new(cell_x(to.x), cell_z(to.y));
    let velocity = (to_w - from_w).normalize_or_zero() * speed;
    commands.spawn((
        Projectile,
        LinearVelocity(velocity),
        Destination(to),
        ExplosionDamage { amount, radius },
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_xyz(from_w.x, 0.4, from_w.y),
        Visibility::default(),
    ));
    info!(
        "[火球] 生成：释放者 ({},{}) → 目标 ({},{})，初始格 ({},{})，速度 {speed} 格/秒，爆炸 {amount} 伤害 / 半径 {radius}",
        from.x, from.y, to.x, to.y, spawn.x, spawn.y
    );
}

/// 消息日志：订阅并打印命中等战斗消息，同时写入 `BattleLog`（屏幕 UI）
pub fn message_log_system(
    mut hits: MessageReader<HitLanded>,
    player_q: Query<Entity, With<Player>>,
    enemy_q: Query<Entity, With<Enemy>>,
    projectile_q: Query<Entity, With<Projectile>>,
    mut log: ResMut<BattleLog>,
) {
    let player = player_q.single().ok();
    let enemy = enemy_q.single().ok();
    let label = |entity: Entity| -> &'static str {
        if Some(entity) == player {
            "玩家"
        } else if Some(entity) == enemy {
            "敌人"
        } else if projectile_q.contains(entity) {
            "火球"
        } else {
            "?"
        }
    };

    for m in hits.read() {
        let msg = format!(
            "[命中] {} → {}：{} 伤害（先手判定 {:?}）",
            label(m.attacker),
            label(m.defender),
            m.damage,
            m.order
        );
        info!("{msg}");
        log.push(msg);
    }
}
