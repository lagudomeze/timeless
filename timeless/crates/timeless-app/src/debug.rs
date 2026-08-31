//! # 应用层：极简调试面板
//!
//! 展示战斗关键状态，并按阶段提供**可用操作列表**：
//! - Decision（暂停）：从 `SKILLS` 表渲染技能按钮（可用性由能力组件过滤），提交 [Commit]
//! - Reaction（暂停）：从 `REACTIONS` 表渲染反应按钮
//! - 任意阶段：重置 [Reset Battle]
//!
//! 面板不直接改状态，只写 Message，由对应系统消费（模块间解耦）。

use bevy::prelude::*;
use bevy_egui::EguiContexts;
use bevy_egui::egui;

use crate::combat::*;
use crate::display::map::GRID_SIZE;
use crate::menu::{
    CanAttack, CanFireball, CanMove, CanRoll, CommitTurn, MenuSelection, REACTIONS, ReactionSelect,
    SKILLS, SelectSkill, available_skills,
};
use crate::movement::Position;
use crate::timeline::*;

/// 调试面板玩家查询（数值 + 位置）
type PlayerDebugQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Position, &'static Health, &'static Stamina), With<Player>>;

/// 调试面板敌人查询（数值 + 位置）
type EnemyDebugQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Position, &'static Health), With<Enemy>>;

/// 调试面板能力查询（玩家可行动标记）
type DebugCapabilityQuery<'w, 's> = Query<
    'w,
    's,
    (Has<CanAttack>, Has<CanMove>, Has<CanRoll>, Has<CanFireball>),
    (With<Player>, Without<Enemy>),
>;

/// 极简调试面板（egui 窗口）
///
/// 参数较多：egui 上下文 + 菜单游标 + 数值/能力查询 + 消息写出器，属合理边界。
#[allow(clippy::too_many_arguments)]
pub fn debug_panel_system(
    mut contexts: EguiContexts,
    tl: Res<TimeLineState>,
    menu: Res<MenuSelection>,
    player_q: PlayerDebugQuery<'_, '_>,
    enemy_q: EnemyDebugQuery<'_, '_>,
    capability_q: DebugCapabilityQuery<'_, '_>,
    actions: ActionLabelQuery<'_, '_>,
    mut ev_select: MessageWriter<SelectSkill>,
    mut ev_commit: MessageWriter<CommitTurn>,
    mut ev_reaction: MessageWriter<ReactionSelect>,
    mut ev_reset: MessageWriter<ResetBattle>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let player_sta = player_q.single().map(|(_, _, _, s)| s.current).ok();
    let available = capability_q
        .single()
        .ok()
        .map(|(a, m, r, f)| available_skills(a, m, r, f))
        .unwrap_or_default();

    egui::Window::new("Combat Debug")
        .default_width(420.0)
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
                Ok((entity, p, h, s)) => {
                    let action = action_label(entity, &actions);
                    ui.label(format!(
                        "Player @({},{})  HP {}/{}  STA {}/{}  act {action}",
                        p.0.x, p.0.y, h.current, h.max, s.current, s.max
                    ));
                }
                Err(_) => {
                    ui.colored_label(egui::Color32::RED, "Player: DEAD");
                }
            }
            match enemy_q.single() {
                Ok((entity, p, h)) => {
                    let action = action_label(entity, &actions);
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
                    ui.label("Available skills (Tab):");
                    ui.horizontal_wrapped(|ui| {
                        for (i, &skill) in available.iter().enumerate() {
                            let def = &SKILLS[skill];
                            let selected = menu.index == i;
                            let can_afford = player_sta.unwrap_or(0) >= def.cost;
                            let label = format!(
                                "{} {}",
                                def.label,
                                if def.cost > 0 {
                                    format!("(-{})", def.cost)
                                } else {
                                    String::new()
                                }
                            );
                            let btn = ui.add_enabled(
                                can_afford,
                                egui::Button::new(label).selected(selected),
                            );
                            if btn.clicked() {
                                ev_select.write(SelectSkill(skill));
                            }
                        }
                    });
                    if ui.button("✅ Commit (Space)").clicked() {
                        ev_commit.write(CommitTurn);
                    }
                }
                TurnPhase::Reaction => {
                    ui.label("Available reactions (Tab):");
                    ui.horizontal_wrapped(|ui| {
                        for (i, def) in REACTIONS.iter().enumerate() {
                            let selected = menu.index == i;
                            let can_afford = player_sta.unwrap_or(0) >= def.cost;
                            let label = format!(
                                "{} {}",
                                def.label,
                                if def.cost > 0 {
                                    format!("(-{})", def.cost)
                                } else {
                                    String::new()
                                }
                            );
                            let btn = ui.add_enabled(
                                can_afford,
                                egui::Button::new(label).selected(selected),
                            );
                            if btn.clicked() {
                                ev_reaction.write(ReactionSelect(i));
                            }
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
                    ui.small("Enemy will hit you — cancel to dodge / parry");
                }
                TurnPhase::Resolving => {
                    ui.label("Resolving… (timeline)");
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

/// 精力是否不足以做任何消耗性反应（< 2 时提示）
fn stamina_too_low(sta: Option<u32>) -> bool {
    sta.is_some_and(|s| s < 2)
}
