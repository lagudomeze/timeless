//! # 时间线领域：动作实体调度 + 回合状态机
//!
//! We-Go 同步回合 + 逻辑刻度时间线：
//!
//! ```text
//! 动作实体（Action Entity）：
//!   生成(Declared) ──finalize──▶ Pending ──scheduler(时间到期)──▶ Committed ──执行器──▶ despawn
//! ```
//!
//! - 行动 = 实体：载荷组件（`Attack` / `MoveTo` / `Roll` / `Fireball` / `Parry`）+
//!   [`ScheduledAction`]，状态标记 `Declared` → `Pending` → `Committed`；
//! - 虚拟时间 [`Time<Virtual>`] 控制时间流动：Decision / Reaction / GameOver 暂停，
//!   Resolving 推进（调度器按 `execute_at` 到期转 `Committed`）；
//! - 回合收尾：无动作 / 投射物残留 → 下一回合 Decision（或 GameOver）。

use bevy::ecs::template::FromTemplate;
use bevy::prelude::*;
use bevy::time::Virtual;

use crate::combat::{BattleLog, Dodging, Enemy, Parrying, Player};
use crate::display::unit::PaperAssets;
use crate::menu::MenuSelection;
use crate::movement::Projectile;

/// 逻辑刻度：1 帧前摇 = TICK_MS 毫秒（`AttackFrame` 换算 `execute_at` 用）
pub const TICK_MS: u64 = 100;

/// 动作实体残留查询（任一状态）
type AnyActionQuery<'w, 's> =
    Query<'w, 's, (), Or<(With<Declared>, With<Pending>, With<Committed>)>>;

/// 动作实体 Entity 查询（重置清场用）
type ActionEntityQuery<'w, 's> =
    Query<'w, 's, Entity, Or<(With<Declared>, With<Pending>, With<Committed>)>>;

/// 回合阶段（We-Go 同步回合；暂停由 `Time<Virtual>` 驱动，阶段只做 UI / 门控）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnPhase {
    /// 决策暂停：等待玩家选择行动（时间冻结）
    Decision,
    /// 反应暂停：双方都将攻击，等待玩家选择（时间冻结）
    Reaction,
    /// 结算：时间流动，动作按 `execute_at` 依次执行
    Resolving,
    /// 战斗结束（时间冻结）
    GameOver,
}

/// 时间线状态（全局资源）
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeLineState {
    pub global_tick: u32,
    pub phase: TurnPhase,
    /// 本回合是否已处理过反应（防止 Resolving 中重复触发 Reaction）
    pub reaction_resolved: bool,
}

impl Default for TimeLineState {
    fn default() -> Self {
        Self {
            global_tick: 0,
            phase: TurnPhase::Decision,
            reaction_resolved: false,
        }
    }
}

// ─────────────────────────── 动作实体调度 ───────────────────────────

/// 动作实体统一调度数据：`execute_at` 由 `finalize_declared_actions` 分配
/// （now + cast_duration），调度器与执行器只读本字段，不感知载荷类型。
#[derive(Component, Debug, Clone, Copy, FromTemplate)]
pub struct ScheduledAction {
    /// 绝对执行时刻（虚拟时间毫秒刻度）
    pub execute_at: u64,
    /// 前摇 / 施法时长（毫秒）
    pub cast_duration: u64,
    /// 执行者（PC / NPC）
    pub actor: Entity,
}

/// 状态标记（ZST）：刚生成，未分配执行时间
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Declared;

/// 状态标记（ZST）：已入队，在时间线上等待执行
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pending;

/// 状态标记（ZST）：已到期，等待结算
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Committed;

// ─────────────────────────── 消息 ───────────────────────────

/// 重置战斗（调试面板按钮 / R 键触发）：清场并重生双方
#[derive(Message, Debug, Clone, Copy)]
pub struct ResetBattle;

/// 提交成功（由 `menu::commit_system` 发出，本文件 `phase_advance_system` 消费）
#[derive(Message, Debug, Clone, Copy)]
pub struct TurnCommitted;

// ─────────────────────────── 系统 ───────────────────────────

/// 初始化：开局 Decision 暂停（虚拟时间冻结，等待第一次决策）
pub fn start_paused(mut time: ResMut<Time<Virtual>>) {
    time.pause();
}

/// 暂停与阶段同步：Decision / Reaction / GameOver 冻结时间，Resolving 放行
pub fn sync_pause_system(tl: Res<TimeLineState>, mut time: ResMut<Time<Virtual>>) {
    let paused = matches!(
        tl.phase,
        TurnPhase::Decision | TurnPhase::Reaction | TurnPhase::GameOver
    );
    if paused {
        time.pause();
    } else {
        time.unpause();
    }
}

/// 清掉指定执行者已声明的动作（决策阶段替换旧选择用）
pub(crate) fn despawn_declared_for(
    commands: &mut Commands,
    actor: Entity,
    q: &Query<(Entity, &ScheduledAction), With<Declared>>,
) {
    for (entity, scheduled) in q {
        if scheduled.actor == actor {
            commands.entity(entity).despawn();
        }
    }
}

/// 战斗重置：R 键或调试面板的 `ResetBattle` 消息触发。
/// 清场（含动作实体 / 投射物）→ 重置状态 → 重生双方。
#[allow(clippy::too_many_arguments)]
pub fn reset_system(
    mut ev_reset: MessageReader<ResetBattle>,
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut tl: ResMut<TimeLineState>,
    mut menu: ResMut<MenuSelection>,
    mut log: ResMut<BattleLog>,
    paper: Res<PaperAssets>,
    player_q: Query<Entity, With<Player>>,
    enemy_q: Query<Entity, With<Enemy>>,
    actions: ActionEntityQuery<'_, '_>,
    projectiles: Query<Entity, With<Projectile>>,
) {
    let requested = keys.just_pressed(KeyCode::KeyR) || ev_reset.read().next().is_some();
    if !requested {
        return;
    }
    for e in player_q.iter().chain(enemy_q.iter()) {
        commands.entity(e).despawn();
    }
    for e in actions.iter().chain(projectiles.iter()) {
        commands.entity(e).despawn();
    }
    *tl = TimeLineState::default();
    menu.index = 0;
    crate::setup::spawn_combatants(&mut commands, &paper);
    log.push("─ 战斗已重置 ─".to_string());
    info!("[重置] 战斗已还原（双方满状态，回合 {}）", tl.global_tick);
}

/// 阶段推进（消费 `TurnCommitted`）：提交通过 → 进入 Resolving（时间放行）
pub fn phase_advance_system(
    mut tl: ResMut<TimeLineState>,
    mut ev_committed: MessageReader<TurnCommitted>,
) {
    if ev_committed.read().next().is_none() {
        return;
    }
    tl.phase = TurnPhase::Resolving;
    tl.reaction_resolved = false;
}

/// 入队：`Declared` → `Pending`，分配 `execute_at = now + cast_duration`
/// （时间暂停时 now 冻结，前摇自然停表）
pub fn finalize_declared_actions(
    tl: Res<TimeLineState>,
    time: Res<Time<Virtual>>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut ScheduledAction), With<Declared>>,
) {
    if tl.phase != TurnPhase::Resolving {
        return;
    }
    let now = time.elapsed().as_millis() as u64;
    for (entity, mut scheduled) in &mut q {
        scheduled.execute_at = now + scheduled.cast_duration;
        commands.entity(entity).remove::<Declared>().insert(Pending);
    }
}

/// 调度：`Pending` → `Committed`（`execute_at` 到期；暂停期间不推进）
pub fn scheduler(
    time: Res<Time<Virtual>>,
    mut commands: Commands,
    q: Query<(Entity, &ScheduledAction), With<Pending>>,
) {
    if time.is_paused() {
        return;
    }
    let now = time.elapsed().as_millis() as u64;
    for (entity, scheduled) in &q {
        if scheduled.execute_at <= now {
            commands
                .entity(entity)
                .remove::<Pending>()
                .insert(Committed);
        }
    }
}

/// 回合收尾：无动作 / 投射物残留 → 清除防御标记，下一回合 Decision（或 GameOver）
#[allow(clippy::too_many_arguments)]
pub fn turn_end_system(
    time: Res<Time<Virtual>>,
    mut tl: ResMut<TimeLineState>,
    mut menu: ResMut<MenuSelection>,
    mut commands: Commands,
    mut log: ResMut<BattleLog>,
    actions: AnyActionQuery<'_, '_>,
    projectiles: Query<(), With<Projectile>>,
    markers: Query<(Entity, Option<&Dodging>, Option<&Parrying>)>,
) {
    if time.is_paused() || tl.phase != TurnPhase::Resolving {
        return;
    }
    if !actions.is_empty() || !projectiles.is_empty() {
        return;
    }
    for (entity, dodging, parrying) in &markers {
        if dodging.is_some() || parrying.is_some() {
            commands
                .entity(entity)
                .remove::<Dodging>()
                .remove::<Parrying>();
        }
    }
    tl.global_tick += 1;
    tl.phase = TurnPhase::Decision;
    menu.index = 0;
    log.push(format!("──── 回合 {} 结算完毕 ────", tl.global_tick));
    info!("[回合] {} 结算完毕，进入下一回合决策", tl.global_tick);
}
