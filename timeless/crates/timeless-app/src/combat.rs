//! # 战斗领域：组件 + 消息 + 系统（一个文件）
//!
//! 按「实体 + 小组件组合」建模（参考 bevy-compose 的 Health/Damage 对与
//! bevy_turn_based_combat 的意图队列思路）：
//! - `Health`（生命）对应 `Damage`（伤害）：攻击方携带 `Damage`，命中时按量扣减防御方 `Health`；
//! - 攻击属性拆成 `AttackFrame`（速度帧）/ `AttackRange`（射程）/ `Impact`（破势）
//!   三个独立组件，裁决前组装成领域层 `AttackStats`，领域层保持零 Bevy 依赖；
//! - 行动用意图组件表达：`AttackIntent`（本回合攻击）、`DodgeActive`（闪躲无敌帧）、
//!   `Parry`（招架，预留）、`Interrupted`（破势打断）；位移意图见 `movement` 领域；
//! - 结算流水线：`ai_system` 生成意图 → `resolve_system` 裁决 + 应用伤害 →
//!   `death_check_system` 清场 → `message_log_system` 打印事件链。

use std::collections::VecDeque;

use bevy::prelude::*;

use timeless_domain::combat::{
    AttackStats as DomainAttackStats, HitOrder, Side, resolve_attack, resolve_combat,
};

use crate::menu::{ActionSubmitted, MenuSelection};
use crate::movement::{
    FireballAssets, FireballCast, MoveIntent, Position, Projectile, RetreatIntent, spawn_fireball,
};
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

/// 精力：翻滚 / 翻滚取消消耗
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

/// 攻击意图：本回合将攻击（决策/反应阶段挂载，结算后移除）
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackIntent;

/// 闪躲（无敌帧）：挂载期间敌方攻击无效；当前由翻滚提供
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DodgeActive;

/// 招架：格挡攻击（Phase 3 预留，先占位组件；届时接入格挡结算）
#[allow(dead_code)]
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

// ─────────────────────────── 消息 ───────────────────────────

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

/// 敌人 AI 查询（意图为 Option，读位置与攻击属性）
type EnemyAiQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        Option<&'static AttackIntent>,
        Option<&'static MoveIntent>,
        &'static AttackFrame,
        &'static AttackRange,
    ),
    With<Enemy>,
>;

/// 敌人 AI：决策阶段开始即生成意图（进入射程锁定攻击，否则逼近一格）。
/// 仅在本回合尚未决策时执行（无 `AttackIntent` / `MoveIntent`），避免重复决策。
/// 必须在 `input_system` 之前运行，确保玩家提交时的威胁判定读到已定的敌人意图。
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
        commands.entity(entity).insert(AttackIntent);
        info!(
            "[AI] 敌人进入射程（距离 {dist}），锁定攻击（帧 {}）",
            frame.0
        );
    } else {
        let target = Position(pos.0.step_toward(player_pos.0));
        commands.entity(entity).insert(MoveIntent { target });
        info!("[AI] 敌人逼近玩家 → {:?}", target.0);
    }
}

/// 玩家 / 敌人战斗查询（意图与闪躲为 Option）
type PlayerCombat<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        &'static mut Health,
        Option<&'static AttackIntent>,
        Option<&'static DodgeActive>,
        Option<&'static Parry>,
        &'static AttackFrame,
        &'static AttackRange,
        &'static Impact,
        &'static Damage,
    ),
    (With<Player>, Without<Enemy>),
>;

type EnemyCombat<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        &'static mut Health,
        Option<&'static AttackIntent>,
        Option<&'static DodgeActive>,
        &'static AttackFrame,
        &'static AttackRange,
        &'static Impact,
        &'static Damage,
    ),
    (With<Enemy>, Without<Player>),
>;

/// 火球施放查询（玩家，含锁定目标格）
type PlayerFireballQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Position, &'static FireballCast),
    (With<Player>, Without<Enemy>),
>;

/// 回合结算（瞬时）：读取双方意图 → 组装领域层属性 → 裁决 → 应用伤害 → 清意图。
/// 位移意图已由 `movement::apply_move_intents_system` 先行处理（改变站位后再裁决）。
#[allow(clippy::too_many_arguments)]
pub fn resolve_system(
    mut tl: ResMut<TimeLineState>,
    mut menu: ResMut<MenuSelection>,
    mut player_q: PlayerCombat<'_, '_>,
    mut enemy_q: EnemyCombat<'_, '_>,
    mut stamina_q: Query<&mut Stamina, (With<Player>, Without<Enemy>)>,
    fireball_q: PlayerFireballQuery<'_, '_>,
    fireball_assets: Res<FireballAssets>,
    mut commands: Commands,
    mut log: ResMut<BattleLog>,
    mut ev_hit: MessageWriter<HitLanded>,
) {
    if tl.phase != TurnPhase::Resolving {
        return;
    }
    let Ok((
        p_entity,
        p_pos,
        mut p_hp,
        p_attack,
        p_dodge,
        p_parry,
        p_frame,
        p_range,
        p_impact,
        p_damage,
    )) = player_q.single_mut()
    else {
        return;
    };
    let Ok((e_entity, e_pos, mut e_hp, e_attack, _e_dodge, e_frame, e_range, e_impact, e_damage)) =
        enemy_q.single_mut()
    else {
        return;
    };

    tl.global_tick += 1;
    info!("════ 回合 {} 结算 ════", tl.global_tick);
    log.push(format!("──── 回合 {} 结算 ────", tl.global_tick));

    let dist = p_pos.0.chebyshev(e_pos.0);
    // 小组件 → 领域层聚合类型（领域层仍保持零 Bevy 依赖的纯函数裁决）
    let p_stats = DomainAttackStats::new(p_frame.0, p_range.0, p_impact.0, p_damage.0);
    let e_stats = DomainAttackStats::new(e_frame.0, e_range.0, e_impact.0, e_damage.0);

    match (p_attack.is_some(), e_attack.is_some()) {
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
                    Side::Attacker => ("玩家", p_entity),
                    Side::Defender => ("敌人", e_entity),
                };
                commands.entity(cancelled).insert(Interrupted);
                info!("[破势] {who} 的攻击被打断！");
                log.push(format!("[破势] {who} 的攻击被打断！"));
            }
            if r.attacker_hits && r.interrupted != Some(Side::Attacker) {
                apply_hit(
                    &mut e_hp,
                    p_entity,
                    e_entity,
                    p_damage.0,
                    r.order,
                    &mut ev_hit,
                );
            }
            if r.defender_hits && r.interrupted != Some(Side::Defender) {
                apply_hit(
                    &mut p_hp,
                    e_entity,
                    p_entity,
                    e_damage.0,
                    r.order,
                    &mut ev_hit,
                );
            }
        }
        (true, false) => {
            if resolve_attack(&p_stats, dist) {
                apply_hit(
                    &mut e_hp,
                    p_entity,
                    e_entity,
                    p_damage.0,
                    HitOrder::AttackerFirst,
                    &mut ev_hit,
                );
            } else {
                info!("[裁决] 玩家攻击落空（距离 {dist} 超出射程 {}）", p_range.0);
                log.push(format!(
                    "[裁决] 玩家攻击落空（距离 {dist} 超出射程 {}）",
                    p_range.0
                ));
            }
        }
        (false, true) => {
            if p_dodge.is_some() {
                info!("[闪避] 玩家翻滚（无敌帧）闪开了敌人的攻击！");
                log.push("[闪避] 玩家翻滚（无敌帧）闪开了敌人的攻击！".to_string());
            } else if p_parry.is_some() {
                info!("[招架] 玩家招架格挡了敌人的攻击！");
                log.push("[招架] 玩家招架格挡了敌人的攻击！".to_string());
            } else if resolve_attack(&e_stats, dist) {
                apply_hit(
                    &mut p_hp,
                    e_entity,
                    p_entity,
                    e_damage.0,
                    HitOrder::DefenderFirst,
                    &mut ev_hit,
                );
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

    // 火球施放：生成投射物（目标格在提交时已锁定，敌人移动即可躲避）
    if let Some(cast) = fireball_q.get(p_entity).ok().map(|(_, _, cast)| *cast) {
        spawn_fireball(
            &mut commands,
            p_pos.0,
            cast.target.0,
            cast.speed,
            cast.amount,
            cast.radius,
            fireball_assets.mesh.clone(),
            fireball_assets.material.clone(),
        );
        info!(
            "[技能] 玩家施放火球 → ({},{})",
            cast.target.0.x, cast.target.0.y
        );
        log.push(format!(
            "[技能] 玩家施放火球 → ({},{})",
            cast.target.0.x, cast.target.0.y
        ));
    }

    // 精力回复：每回合结算后 +1（上限）
    if let Ok(mut stamina) = stamina_q.get_mut(p_entity)
        && stamina.current < stamina.max
    {
        stamina.current += 1;
        info!("[恢复] 精力 +1（{}/{}）", stamina.current, stamina.max);
        log.push(format!(
            "[恢复] 精力 +1（{}/{}）",
            stamina.current, stamina.max
        ));
    }

    // 清除本回合战斗意图（位移意图已由 movement 移除；`Interrupted` 亦随回合结束清掉）
    commands
        .entity(p_entity)
        .remove::<AttackIntent>()
        .remove::<DodgeActive>()
        .remove::<Interrupted>()
        .remove::<Parry>()
        .remove::<FireballCast>();
    commands
        .entity(e_entity)
        .remove::<AttackIntent>()
        .remove::<DodgeActive>()
        .remove::<Interrupted>();

    if p_hp.is_alive() && e_hp.is_alive() {
        menu.index = 0; // 回到决策暂停，游标归零
        tl.phase = TurnPhase::Decision;
    }
    // 有阵亡：保持 Resolving，由 death_check_system 收尾并切到 GameOver
}

/// 死亡检查：血量归零的实体统一清场；战斗单位阵亡时切换 GameOver。
/// 与 `resolve_system` 解耦——投射物（火球）在任意阶段炸死目标也能正确收尾。
pub fn death_check_system(
    mut tl: ResMut<TimeLineState>,
    mut commands: Commands,
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
        info!("══ 战斗结束（回合 {}）══", tl.global_tick);
    }
}

/// 应用伤害并广播 HitLanded（输出由 message_log_system 消费；投射物爆炸复用）
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

/// 意图组件 → 简短中文描述（HUD / 调试面板共用）
pub(crate) fn intent_label(
    attack: Option<&AttackIntent>,
    mov: Option<&MoveIntent>,
    retreat: Option<&RetreatIntent>,
    parry: Option<&Parry>,
    fireball: Option<&FireballCast>,
) -> String {
    if parry.is_some() {
        "招架".to_string()
    } else if fireball.is_some() {
        "火球".to_string()
    } else if retreat.is_some() {
        "翻滚".to_string()
    } else if let Some(m) = mov {
        format!("移动 → ({},{})", m.target.0.x, m.target.0.y)
    } else if attack.is_some() {
        "攻击".to_string()
    } else {
        "无".to_string()
    }
}

/// 消息日志：订阅并打印广播的战斗消息（演示模块间解耦通信）
#[allow(clippy::too_many_arguments)]
pub fn message_log_system(
    mut submits: MessageReader<ActionSubmitted>,
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

    for m in submits.read() {
        info!("[提交] {} 指令 {:?}", label(m.entity), m.action);
        log.push(format!("[提交] {} 指令 {:?}", label(m.entity), m.action));
    }
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
