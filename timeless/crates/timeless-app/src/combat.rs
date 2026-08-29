//! # 战斗领域：组件 + 消息 + 系统（一个文件）
//!
//! 行动即组件：`Attack` / `Move` / `Roll` / `Fireball` 直接挂在实体上，
//! 决策阶段插入、结算后移除；没有行动组件的单位视为「待机」。
//!
//! 结算流水线（独立 ECS 系统，按顺序执行）：
//!
//! ```text
//! ai_system（AI 插入行动组件）
//!   → movement::apply_move_intents_system（Move/Roll 位移先行）
//!   → resolve_system（按 帧→射程→破势 裁决，产出 HitPending）
//!   → dodge_system（Roll → 闪避，独立系统）
//!   → parry_system（Parry → 招架 + 反制，独立系统）
//!   → damage_system（应用剩余伤害，产出 HitLanded）
//!   → death_check_system（清场 + 阶段推进）→ message_log_system
//! ```

use std::collections::VecDeque;

use bevy::prelude::*;

use timeless_domain::combat::{
    AttackStats as DomainAttackStats, HitOrder, Side, resolve_attack, resolve_combat,
};
use timeless_domain::grid::GridPos;

use crate::menu::MenuSelection;
use crate::movement::{Fireball, FireballAssets, Move, Position, Projectile, Roll, spawn_fireball};
use crate::timeline::{TimeLineState, TurnPhase};

// ─────────────────────────── 组件（状态） ───────────────────────────

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

/// 单次攻击伤害（与 `Health` 对应的输出侧组件；命中时按此扣减）
#[derive(Component, Debug, Clone, Copy)]
pub struct Damage(pub u32);

/// 攻击帧：数值越小，出招越快、越先命中
#[derive(Component, Debug, Clone, Copy)]
pub struct AttackFrame(pub u32);

/// 攻击射程（网格距离，按切比雪夫距离计）
#[derive(Component, Debug, Clone, Copy)]
pub struct AttackRange(pub u32);

/// 破势：双方完全同时命中时，破势高者打断对方攻击
#[derive(Component, Debug, Clone, Copy)]
pub struct Impact(pub u32);

/// 精力：翻滚 / 翻滚取消 / 招架 / 火球消耗
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

/// 攻击行动（标记）：本回合将攻击（决策/反应阶段挂载，结算后移除）
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attack;

/// 招架状态：挂载期间敌方攻击被格挡并触发反制（结算后移除）
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parry;

/// 破势打断：本回合该方攻击被取消（不掉伤害）
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interrupted;

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

// ─────────────────────────── 消息（结算消息链） ───────────────────────────

/// 已排定的命中（裁决已通过，等待闪避/招架过滤后由 damage_system 应用）
#[derive(Message, Debug, Clone, Copy)]
pub struct HitPending {
    pub attacker: Entity,
    pub defender: Entity,
    pub damage: u32,
    pub order: HitOrder,
}

/// 通过闪避过滤的命中（dodge_system → parry_system）
#[derive(Message, Debug, Clone, Copy)]
pub struct HitPostDodge {
    pub attacker: Entity,
    pub defender: Entity,
    pub damage: u32,
    pub order: HitOrder,
}

/// 通过招架过滤的命中（parry_system → damage_system）
#[derive(Message, Debug, Clone, Copy)]
pub struct HitPostParry {
    pub attacker: Entity,
    pub defender: Entity,
    pub damage: u32,
    pub order: HitOrder,
}

/// 招架反制：成功格挡后对原攻击方反弹一半伤害
#[derive(Message, Debug, Clone, Copy)]
pub struct CounterHit {
    pub source: Entity,
    pub target: Entity,
    pub damage: u32,
}

/// 一次命中（伤害已应用，供日志 / 后续特效订阅）
#[derive(Message, Debug, Clone, Copy)]
pub struct HitLanded {
    pub attacker: Entity,
    pub defender: Entity,
    pub damage: u32,
    pub order: HitOrder,
}

/// 翻滚取消已执行（玩家独有特权）
#[derive(Message, Debug, Clone, Copy)]
pub struct RollExecuted {
    pub entity: Entity,
}

/// 招架反应已执行（玩家独有特权）
#[derive(Message, Debug, Clone, Copy)]
pub struct ParryExecuted {
    pub entity: Entity,
}

// ─────────────────────────── 系统 ───────────────────────────

/// 敌人 AI 查询（读位置、行动组件与攻击属性）
type EnemyAiQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        Option<&'static Attack>,
        Option<&'static Move>,
        &'static AttackFrame,
        &'static AttackRange,
    ),
    With<Enemy>,
>;

/// 敌人 AI：决策阶段开始即插入行动组件（进入射程锁定攻击，否则逼近一格）。
/// 仅在本回合尚未决策时执行（无 `Attack` / `Move`），避免重复决策。
/// 必须在 `input_system` 之前运行，确保玩家提交时的威胁判定读到已定的敌人行动。
pub fn ai_system(
    tl: Res<TimeLineState>,
    mut commands: Commands,
    enemy_q: EnemyAiQuery<'_, '_>,
    player_q: Query<&Position, (With<Player>, Without<Enemy>)>,
) {
    if tl.phase != TurnPhase::Decision {
        return;
    }
    let Ok((entity, pos, attack, mov, frame, range)) = enemy_q.single() else {
        return;
    };
    if attack.is_some() || mov.is_some() {
        return; // 本回合已决策
    }
    let Ok(player_pos) = player_q.single() else {
        return;
    };

    let dist = pos.0.chebyshev(player_pos.0);
    if dist <= range.0 {
        commands.entity(entity).insert(Attack);
        info!(
            "[AI] 敌人进入射程（距离 {dist}），锁定攻击（帧 {}）",
            frame.0
        );
    } else {
        let target = Position(pos.0.step_toward(player_pos.0));
        commands.entity(entity).insert(Move { target });
        info!("[AI] 敌人逼近玩家 → {:?}", target.0);
    }
}

/// 战斗单位快照（读时收集，避免长生命周期借用）
#[derive(Clone, Copy)]
struct Combatant {
    entity: Entity,
    pos: GridPos,
    is_player: bool,
    attacking: bool,
    frame: u32,
    range: u32,
    impact: u32,
    damage: u32,
    fireball: Option<Fireball>,
}

/// 战斗单位查询（玩家 / 敌人统一收集；行动组件均为 Option）
type CombatantQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        Option<&'static Attack>,
        Option<&'static Roll>,
        Option<&'static Fireball>,
        &'static AttackFrame,
        &'static AttackRange,
        &'static Impact,
        &'static Damage,
        Option<&'static Parry>,
    ),
    Or<(With<Player>, With<Enemy>)>,
>;

/// 回合结算（瞬时）：收集所有战斗单位 → 位移已先行 → 按 帧→射程→破势 裁决 →
/// 产出 `HitPending`（伤害由后续独立系统应用）→ 火球生成 → 精力回复 → 清行动组件。
///
/// 没有行动组件的单位即「待机」，不参与交锋。
#[allow(clippy::too_many_arguments)]
pub fn resolve_system(
    mut tl: ResMut<TimeLineState>,
    combatant_q: CombatantQuery<'_, '_>,
    player_q: Query<Entity, (With<Player>, Without<Enemy>)>,
    mut stamina_q: Query<&mut Stamina, (With<Player>, Without<Enemy>)>,
    fireball_assets: Res<FireballAssets>,
    mut commands: Commands,
    mut log: ResMut<BattleLog>,
    mut ev_pending: MessageWriter<HitPending>,
) {
    if tl.phase != TurnPhase::Resolving {
        return;
    }

    let player_entity = player_q.single().ok();
    let combatants: Vec<Combatant> = combatant_q
        .iter()
        .map(
            |(entity, pos, attack, _roll, fireball, frame, range, impact, damage, _parry)| {
                Combatant {
                    entity,
                    pos: pos.0,
                    is_player: Some(entity) == player_entity,
                    attacking: attack.is_some(),
                    frame: frame.0,
                    range: range.0,
                    impact: impact.0,
                    damage: damage.0,
                    fireball: fireball.copied(),
                }
            },
        )
        .collect();

    let Some(p) = combatants.iter().find(|c| c.is_player) else {
        return;
    };
    let Some(e) = combatants.iter().find(|c| !c.is_player) else {
        return;
    };

    tl.global_tick += 1;
    info!("════ 回合 {} 结算 ════", tl.global_tick);
    log.push(format!("──── 回合 {} 结算 ────", tl.global_tick));

    let dist = p.pos.chebyshev(e.pos);
    let p_stats = DomainAttackStats::new(p.frame, p.range, p.impact, p.damage);
    let e_stats = DomainAttackStats::new(e.frame, e.range, e.impact, e.damage);

    match (p.attacking, e.attacking) {
        (true, true) => {
            let r = resolve_combat(&p_stats, &e_stats, dist);
            let order_str = match r.order {
                HitOrder::AttackerFirst => "玩家先手",
                HitOrder::DefenderFirst => "敌人先手",
                HitOrder::Simultaneous => "同时命中",
            };
            info!("[裁决] {order_str}（距离 {dist}）");
            log.push(format!("[裁决] {order_str}（距离 {dist}）"));

            // 破势打断：被打断一方挂 `Interrupted`，攻击取消（不掉伤害）
            if let Some(side) = r.interrupted {
                let (who, cancelled) = match side {
                    Side::Attacker => ("玩家", p.entity),
                    Side::Defender => ("敌人", e.entity),
                };
                commands.entity(cancelled).insert(Interrupted);
                info!("[破势] {who} 的攻击被打断！");
                log.push(format!("[破势] {who} 的攻击被打断！"));
            }
            if r.attacker_hits && r.interrupted != Some(Side::Attacker) {
                ev_pending.write(HitPending {
                    attacker: p.entity,
                    defender: e.entity,
                    damage: p.damage,
                    order: r.order,
                });
            }
            if r.defender_hits && r.interrupted != Some(Side::Defender) {
                ev_pending.write(HitPending {
                    attacker: e.entity,
                    defender: p.entity,
                    damage: e.damage,
                    order: r.order,
                });
            }
        }
        (true, false) => {
            if resolve_attack(&p_stats, dist) {
                ev_pending.write(HitPending {
                    attacker: p.entity,
                    defender: e.entity,
                    damage: p.damage,
                    order: HitOrder::AttackerFirst,
                });
            } else {
                info!("[裁决] 玩家攻击落空（距离 {dist} 超出射程 {}）", p.range);
                log.push(format!(
                    "[裁决] 玩家攻击落空（距离 {dist} 超出射程 {}）",
                    p.range
                ));
            }
        }
        (false, true) => {
            if resolve_attack(&e_stats, dist) {
                ev_pending.write(HitPending {
                    attacker: e.entity,
                    defender: p.entity,
                    damage: e.damage,
                    order: HitOrder::DefenderFirst,
                });
            } else {
                info!("[裁决] 敌人攻击落空（距离 {dist}）");
                log.push(format!("[裁决] 敌人攻击落空（距离 {dist}）"));
            }
        }
        (false, false) => {
            info!("[裁决] 本回合无交锋");
            log.push("[裁决] 本回合无交锋".to_string());
        }
    }

    // 火球行动：生成投射物（目标格在提交时已锁定，敌人移动即可躲避）
    if let Some(fb) = p.fireball {
        spawn_fireball(
            &mut commands,
            p.pos,
            fb.target.0,
            fb.speed,
            fb.amount,
            fb.radius,
            fireball_assets.mesh.clone(),
            fireball_assets.material.clone(),
        );
        info!(
            "[技能] 玩家施放火球 → ({},{})",
            fb.target.0.x, fb.target.0.y
        );
        log.push(format!(
            "[技能] 玩家施放火球 → ({},{})",
            fb.target.0.x, fb.target.0.y
        ));
    }

    // 精力回复：每回合结算后 +1（上限）
    if let Ok(mut stamina) = stamina_q.get_mut(p.entity)
        && stamina.current < stamina.max
    {
        stamina.current += 1;
        info!("[恢复] 精力 +1（{}/{}）", stamina.current, stamina.max);
        log.push(format!(
            "[恢复] 精力 +1（{}/{}）",
            stamina.current, stamina.max
        ));
    }

    // 清除本回合行动与状态组件（位移行动中的 Move 已由 movement 移除）
    for c in &combatants {
        commands
            .entity(c.entity)
            .remove::<Attack>()
            .remove::<Move>()
            .remove::<Roll>()
            .remove::<Fireball>()
            .remove::<Parry>()
            .remove::<Interrupted>();
    }
}

/// 闪避系统（独立）：`Roll` 行动存在 = 本回合闪避，拦截所有指向该单位的命中
pub fn dodge_system(
    mut pending: MessageReader<HitPending>,
    mut survivors: MessageWriter<HitPostDodge>,
    defenders: Query<Option<&Roll>>,
    mut log: ResMut<BattleLog>,
) {
    for hit in pending.read() {
        if defenders.get(hit.defender).ok().flatten().is_some() {
            info!("[闪避] 翻滚中的单位闪开了攻击！");
            log.push("[闪避] 翻滚中的单位闪开了攻击！".to_string());
            // 不转发 → 命中被闪避
        } else {
            survivors.write(HitPostDodge {
                attacker: hit.attacker,
                defender: hit.defender,
                damage: hit.damage,
                order: hit.order,
            });
        }
    }
}

/// 招架系统（独立）：`Parry` 状态存在 = 格挡本次攻击并反弹一半伤害（反制）
pub fn parry_system(
    mut incoming: MessageReader<HitPostDodge>,
    mut survivors: MessageWriter<HitPostParry>,
    mut counters: MessageWriter<CounterHit>,
    defenders: Query<Option<&Parry>>,
    mut log: ResMut<BattleLog>,
) {
    for hit in incoming.read() {
        if defenders.get(hit.defender).ok().flatten().is_some() {
            let counter = hit.damage.div_ceil(2);
            info!("[招架] 格挡成功，反制 {counter} 伤害！");
            log.push(format!("[招架] 格挡成功，反制 {counter} 伤害！"));
            counters.write(CounterHit {
                source: hit.defender,
                target: hit.attacker,
                damage: counter,
            });
            // 不转发原命中 → 攻击被格挡
        } else {
            survivors.write(HitPostParry {
                attacker: hit.attacker,
                defender: hit.defender,
                damage: hit.damage,
                order: hit.order,
            });
        }
    }
}

/// 伤害应用系统：应用过滤后的命中与招架反制，产出 `HitLanded`
pub fn damage_system(
    mut hits: MessageReader<HitPostParry>,
    mut counters: MessageReader<CounterHit>,
    mut health_q: Query<&mut Health>,
    mut ev_hit: MessageWriter<HitLanded>,
) {
    for h in hits.read() {
        if let Ok(mut hp) = health_q.get_mut(h.defender) {
            apply_hit(
                &mut hp,
                h.attacker,
                h.defender,
                h.damage,
                h.order,
                &mut ev_hit,
            );
        }
    }
    for c in counters.read() {
        if let Ok(mut hp) = health_q.get_mut(c.target) {
            apply_hit(
                &mut hp,
                c.source,
                c.target,
                c.damage,
                HitOrder::Simultaneous,
                &mut ev_hit,
            );
        }
    }
}

/// 死亡检查 + 阶段推进：血量归零的实体统一清场；
/// 结算完成后按存活情况切回 Decision 或 GameOver（火球任意阶段击杀也能正确收尾）。
pub fn death_check_system(
    mut tl: ResMut<TimeLineState>,
    mut menu: ResMut<MenuSelection>,
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
    if tl.phase == TurnPhase::Resolving {
        if p_alive && e_alive {
            menu.index = 0;
            tl.phase = TurnPhase::Decision;
        } else {
            tl.phase = TurnPhase::GameOver;
            log.push("── 战斗结束 ──".to_string());
            info!("══ 战斗结束（回合 {}）══", tl.global_tick);
        }
    } else if !p_alive || !e_alive {
        // 非结算阶段（如火球飞行途中）阵亡 → 直接结束
        tl.phase = TurnPhase::GameOver;
        log.push("── 战斗结束 ──".to_string());
        info!("══ 战斗结束（回合 {}）══", tl.global_tick);
    }
}

/// 应用伤害并广播 HitLanded（投射物爆炸与近战命中共用同一入口）
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

/// 行动组件 → 简短中文描述（HUD / 调试面板共用）
pub(crate) fn intent_label(
    attack: Option<&Attack>,
    mov: Option<&Move>,
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
    } else if let Some(m) = mov {
        format!("移动 → ({},{})", m.target.0.x, m.target.0.y)
    } else if attack.is_some() {
        "攻击".to_string()
    } else {
        "待机".to_string()
    }
}

/// 消息日志：订阅并打印广播的战斗消息，同时写入 `BattleLog`（屏幕 UI）
#[allow(clippy::too_many_arguments)]
pub fn message_log_system(
    mut hits: MessageReader<HitLanded>,
    mut rolls: MessageReader<RollExecuted>,
    mut parries: MessageReader<ParryExecuted>,
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
    for m in rolls.read() {
        info!("[翻滚取消] {} 中断攻击转为翻滚", label(m.entity));
        log.push(format!("[翻滚取消] {} 中断攻击转为翻滚", label(m.entity)));
    }
    for m in parries.read() {
        info!("[招架] {} 转为招架姿态", label(m.entity));
        log.push(format!("[招架] {} 转为招架姿态", label(m.entity)));
    }
}
