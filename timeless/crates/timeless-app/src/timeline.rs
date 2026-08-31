//! # 时间线领域：动作实体调度（虚拟时间驱动，无回合）
//!
//! 战斗不再有「回合 / 阶段」概念：`Time<Virtual>` 持续流动，所有行动都是
//! 独立实体（载荷组件 + [`ScheduledAction`]），按虚拟时间推进：
//!
//! ```text
//! 玩家草案(Declared) ──提交──▶ Pending(execute_at = now + 前摇) ──scheduler 到期──▶ Committed ──执行器──▶ despawn
//! AI 动作直接入队 Pending
//! ```
//!
//! - 玩家选择技能只生成可覆盖的 `Declared` 草案；`menu::commit_system` 校验 / 扣费后写
//!   `ActionsCommitted`，本文件 `finalize_declared_actions` 落地入队并分配 `execute_at`；
//! - AI 意图由 `combat::ai_system` 在动作清空后直接生成 `Pending` 动作；
//! - 虚拟时间默认流动，只有「战斗结束」等全局停顿才 `Time<Virtual>::pause()`；
//!   暂停期间 `now` 冻结，调度自然停表。

use bevy::ecs::template::FromTemplate;
use bevy::prelude::*;
use bevy::time::Virtual;

use crate::combat::{BattleLog, Enemy, Player};
use crate::display::unit::PaperAssets;
use crate::menu::MenuSelection;
use crate::movement::Projectile;

/// 逻辑刻度：1 帧前摇 = TICK_MS 毫秒（`AttackFrame` 换算 `execute_at` 用）
pub const TICK_MS: u64 = 100;

/// 动作实体 Entity 查询（重置清场用）
type ActionEntityQuery<'w, 's> =
    Query<'w, 's, Entity, Or<(With<Declared>, With<Pending>, With<Committed>)>>;

/// 动作实体统一调度数据：`execute_at` 由入队时分配
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

/// 状态标记（ZST）：玩家草案，未入队（可覆盖）
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

/// 玩家行动已提交（由 `menu::commit_system` 发出，本文件
/// `finalize_declared_actions` 消费）：把玩家全部 `Declared` 草案入队为 `Pending`。
#[derive(Message, Debug, Clone, Copy)]
pub struct ActionsCommitted;

// ─────────────────────────── 系统 ───────────────────────────

/// 清掉指定执行者已声明的动作（选择新草案时替换旧草案用）
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
/// 清场（含动作实体 / 投射物）→ 恢复虚拟时间 → 重生双方。
#[allow(clippy::too_many_arguments)]
pub fn reset_system(
    mut ev_reset: MessageReader<ResetBattle>,
    keys: Res<ButtonInput<KeyCode>>,
    mut time: ResMut<Time<Virtual>>,
    mut commands: Commands,
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
    time.unpause();
    menu.index = 0;
    crate::setup::spawn_combatants(&mut commands, &paper);
    log.push("─ 战斗已重置 ─".to_string());
    info!("[重置] 战斗已还原（双方满状态）");
}

/// 入队：`Declared` → `Pending`，分配 `execute_at = now + cast_duration`
/// （虚拟时间暂停时 now 冻结，前摇自然停表）。消费 `ActionsCommitted`。
pub fn finalize_declared_actions(
    time: Res<Time<Virtual>>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut ScheduledAction), With<Declared>>,
    mut ev_committed: MessageReader<ActionsCommitted>,
) {
    if ev_committed.read().next().is_none() {
        return;
    }
    let now = time.elapsed().as_millis() as u64;
    for (entity, mut scheduled) in &mut q {
        scheduled.execute_at = now + scheduled.cast_duration;
        commands.entity(entity).remove::<Declared>().insert(Pending);
    }
}

/// 调度：`Pending` → `Committed`（`execute_at` 到期；虚拟时间暂停期间不推进）
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
