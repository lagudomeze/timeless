//! # 时间线领域：状态 + 消息 + 系统（一个文件）
//!
//! We-Go 同步时间线数据流：
//!
//! ```text
//! Decision（暂停，选行动） ──提交──▶ 威胁检测
//!    │                                 ├─ 双方攻击 → Reaction（暂停，选继续/翻滚取消）
//!    │                                 └─ 否则     → Resolving（瞬时）
//!    ▼
//! Resolving（瞬时结算） ──▶ 回到 Decision / GameOver
//! ```
//!
//! 本文件包含回合阶段机（`TurnPhase` / `TimeLineState`）、
//! 重置消息与 `reset_system`。

use bevy::prelude::*;

use crate::combat::{BattleLog, Enemy, Player};
use crate::display::unit::PaperAssets;
use crate::menu::MenuSelection;

// ─────────────────────────── 状态 ───────────────────────────

/// 回合阶段（We-Go 同步时间线；非实时——每个暂停点都冻结等待）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnPhase {
    /// 决策暂停：等待玩家选择行动（一切冻结）
    Decision,
    /// 反应暂停：玩家与敌人都将攻击，等待玩家选择（继续 / 翻滚取消）
    Reaction,
    /// 结算：瞬时完成，无等待
    Resolving,
    /// 战斗结束
    GameOver,
}

/// 时间线状态（全局资源）：
/// - `global_tick`：已推进的回合数；
/// - `phase`：当前回合阶段；
/// - `reaction_input_locked`：进入 Reaction 的当帧锁（防止提交用的
///   Space/Q 被 reaction_system 同帧误消费，导致暂停被瞬间跳过）。
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeLineState {
    pub global_tick: u32,
    pub phase: TurnPhase,
    pub reaction_input_locked: bool,
}

impl Default for TimeLineState {
    fn default() -> Self {
        Self {
            global_tick: 0,
            phase: TurnPhase::Decision, // 开局即等待第一次决策
            reaction_input_locked: false,
        }
    }
}

// ─────────────────────────── 消息 ───────────────────────────

/// 重置战斗（调试面板按钮 / R 键触发）：清场并重生双方
#[derive(Message, Debug, Clone, Copy)]
pub struct ResetBattle;

// ─────────────────────────── 系统 ───────────────────────────

/// 战斗重置：R 键或调试面板的 `ResetBattle` 消息触发。
/// 清场 → 重置状态 → 重生双方（出生点与初始数值）。
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
) {
    let requested = keys.just_pressed(KeyCode::KeyR) || ev_reset.read().next().is_some();
    if !requested {
        return;
    }
    for e in player_q.iter() {
        commands.entity(e).despawn();
    }
    for e in enemy_q.iter() {
        commands.entity(e).despawn();
    }
    *tl = TimeLineState::default();
    menu.index = 0;
    crate::setup::spawn_combatants(&mut commands, &paper);
    log.push("─ 战斗已重置 ─".to_string());
    info!("[重置] 战斗已还原（双方满状态，回合 {}）", tl.global_tick);
}
