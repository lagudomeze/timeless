//! # 应用层：极简调试面板
//!
//! 展示战斗关键状态，并按阶段提供**可用操作列表**：
//! - Decision（暂停）：选择行动 [Attack] [Move] [Roll] [Fireball]，提交 [Commit]
//! - Reaction（暂停）：选择反应 [Continue] [Roll-Cancel] [Parry]
//! - 任意阶段：重置 [Reset Battle]
//!
//! 面板不直接改状态，只写 Message，由对应系统消费（模块间解耦）。

use bevy::prelude::*;
use bevy_egui::EguiContexts;
use bevy_egui::egui;

use crate::combat::*;
use crate::display::map::GRID_SIZE;
use crate::menu::*;
use crate::movement::{FireballCast, MoveIntent, Position, RetreatIntent};
use crate::timeline::*;

/// 调试面板玩家查询（数值 + 意图组合）
type PlayerDebugQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static Health,
        &'static Stamina,
        Option<&'static AttackIntent>,
        Option<&'static MoveIntent>,
        Option<&'static RetreatIntent>,
        Option<&'static DodgeActive>,
        Option<&'static Parry>,
        Option<&'static FireballCast>,
    ),
    With<Player>,
>;

/// 调试面板敌人查询（数值 + 意图组合）
type EnemyDebugQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static Health,
        Option<&'static AttackIntent>,
        Option<&'static MoveIntent>,
        Option<&'static RetreatIntent>,
    ),
    With<Enemy>,
>;

/// 极简调试面板（egui 窗口）
///
/// 参数较多：1 个 egui 上下文 + 菜单游标 + 资源/命令 + 4 个只读查询 + 4 个消息写出器，
/// 属合理边界。
#[allow(clippy::too_many_arguments)]
pub fn debug_panel_system(
    mut contexts: EguiContexts,
    tl: Res<TimeLineState>,
    menu: Res<MenuSelection>,
    player_q: PlayerDebugQuery<'_, '_>,
    enemy_q: EnemyDebugQuery<'_, '_>,
    mut ev_select: MessageWriter<SelectAction>,
    mut ev_commit: MessageWriter<CommitTurn>,
    mut ev_reaction: MessageWriter<ReactionSelect>,
    mut ev_reset: MessageWriter<ResetBattle>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let player_sta = player_q.single().map(|(_, _, s, ..)| s.current).ok();

    egui::Window::new("Combat Debug")
        .default_width(380.0)
        .show(ctx, |ui| {
            ui.heading(format!("Tick {}", tl.global_tick));
            let phase_str = match tl.phase {
                TurnPhase::Decision => "DECISION (paused)",
                TurnPhase::Reaction => "REACTION (paused)",
                TurnPhase::Resolving => "RESOLVING",
                TurnPhase::GameOver => "GAME OVER",
            };
            ui.label(format!("Phase: {phase_str}"));
            ui.separator();

            // 双方状态
            match player_q.single() {
                Ok((p, h, s, attack, mov, retreat, dodge, parry, fireball)) => {
                    let action = intent_label(attack, mov, retreat, parry, fireball);
                    let dodge_tag = if dodge.is_some() { " [DODGE]" } else { "" };
                    ui.label(format!(
                        "Player @({},{})  HP {}/{}  STA {}/{}  act {action}{dodge_tag}",
                        p.0.x, p.0.y, h.current, h.max, s.current, s.max
                    ));
                }
                Err(_) => {
                    ui.colored_label(egui::Color32::RED, "Player: DEAD");
                }
            }
            match enemy_q.single() {
                Ok((p, h, attack, mov, retreat)) => {
                    let action = intent_label(attack, mov, retreat, None, None);
                    ui.label(format!(
                        "Enemy  @({},{})  HP {}/{}  act {action}",
                        p.0.x, p.0.y, h.current, h.max
                    ));
                }
                Err(_) => {
                    ui.colored_label(egui::Color32::RED, "Enemy: DEFEATED");
                }
            }
            ui.separator();

            // 按阶段给出可用操作列表（选中高亮跟随键盘 Tab 游标）
            match tl.phase {
                TurnPhase::Decision => {
                    ui.label("Available actions (Tab):");
                    ui.horizontal(|ui| {
                        let is_attack = DECISION_OPTIONS[menu.index] == Action::Attack;
                        if ui.selectable_label(is_attack, "⚔ Attack").clicked() {
                            ev_select.write(SelectAction(Action::Attack));
                        }
                        let is_move = DECISION_OPTIONS[menu.index] == Action::Move;
                        if ui.selectable_label(is_move, "➜ Move").clicked() {
                            ev_select.write(SelectAction(Action::Move));
                        }
                        let is_roll = DECISION_OPTIONS[menu.index] == Action::Roll;
                        if ui.selectable_label(is_roll, "🌀 Roll").clicked() {
                            ev_select.write(SelectAction(Action::Roll));
                        }
                        let can_fireball = player_sta.unwrap_or(0) >= FIREBALL_COST;
                        let is_fireball = DECISION_OPTIONS[menu.index] == Action::Fireball;
                        let btn = ui.add_enabled(
                            can_fireball,
                            egui::Button::new("🔥 Fireball (-2 STA)").selected(is_fireball),
                        );
                        if btn.clicked() {
                            ev_select.write(SelectAction(Action::Fireball));
                        }
                    });
                    if ui.button("✅ Commit (Space)").clicked() {
                        ev_commit.write(CommitTurn);
                    }
                }
                TurnPhase::Reaction => {
                    ui.label("Available reactions (Tab):");
                    ui.horizontal(|ui| {
                        let is_continue = REACTION_OPTIONS[menu.index] == ReactionChoice::Continue;
                        if ui
                            .selectable_label(is_continue, "✅ Continue attack")
                            .clicked()
                        {
                            ev_reaction.write(ReactionSelect(ReactionChoice::Continue));
                        }
                        let can_cancel = player_sta.unwrap_or(0) >= 2;
                        let is_cancel = REACTION_OPTIONS[menu.index] == ReactionChoice::RollCancel;
                        let btn = ui.add_enabled(
                            can_cancel,
                            egui::Button::new("🌀 Roll-Cancel (-2 STA)").selected(is_cancel),
                        );
                        if btn.clicked() {
                            ev_reaction.write(ReactionSelect(ReactionChoice::RollCancel));
                        }
                        let can_parry = player_sta.unwrap_or(0) >= PARRY_COST;
                        let is_parry = REACTION_OPTIONS[menu.index] == ReactionChoice::Parry;
                        let btn = ui.add_enabled(
                            can_parry,
                            egui::Button::new("🛡 Parry (-1 STA)").selected(is_parry),
                        );
                        if btn.clicked() {
                            ev_reaction.write(ReactionSelect(ReactionChoice::Parry));
                        }
                    });
                    if stamina_too_low(player_sta) {
                        ui.colored_label(
                            egui::Color32::YELLOW,
                            format!(
                                "Stamina too low ({}), cannot cancel / parry",
                                player_sta.unwrap_or(0)
                            ),
                        );
                    }
                    ui.small("Enemy will hit you — cancel to dodge");
                }
                TurnPhase::Resolving => {
                    ui.label("Resolving… (instant)");
                }
                TurnPhase::GameOver => {
                    ui.colored_label(egui::Color32::RED, "Battle finished");
                }
            }
            ui.separator();

            ui.small(format!("Grid {}×{}", GRID_SIZE, GRID_SIZE));

            if ui.button("🔄 Reset Battle (also R)").clicked() {
                ev_reset.write(ResetBattle);
            }
        });
}

/// 精力是否不足以翻滚取消（< 2 时禁用该选项并提示）
fn stamina_too_low(sta: Option<u32>) -> bool {
    sta.is_some_and(|s| s < 2)
}
