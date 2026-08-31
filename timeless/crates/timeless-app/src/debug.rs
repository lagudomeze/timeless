//! # 应用层：极简调试面板
//!
//! 展示战斗关键状态，并提供可用操作列表：
//! - 技能按钮（可用性由能力组件过滤）+ 提交 [Commit]
//! - 实时反应按钮（翻滚取消 / 招架，前摇窗口内有效）
//! - 任意时刻：重置 [Reset Battle]
//!
//! 面板不直接改状态，只写 Message，由对应系统消费（模块间解耦）。

use bevy::prelude::*;
use bevy::time::Virtual;
use bevy_egui::EguiContexts;
use bevy_egui::egui;

use crate::combat::*;
use crate::display::map::GRID_SIZE;
use crate::menu::{
    CanAttack, CanFireball, CanMove, CanRoll, CommitAction, MenuSelection, ReactionInput,
    ReactionKind, SKILLS, SelectSkill, available_skills,
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
/// 参数较多：egui 上下文 + 虚拟时间 + 菜单游标 + 数值/能力查询 + 消息写出器，属合理边界。
#[allow(clippy::too_many_arguments)]
pub fn debug_panel_system(
    mut contexts: EguiContexts,
    time: Res<Time<Virtual>>,
    menu: Res<MenuSelection>,
    player_q: PlayerDebugQuery<'_, '_>,
    enemy_q: EnemyDebugQuery<'_, '_>,
    capability_q: DebugCapabilityQuery<'_, '_>,
    actions: ActionLabelQuery<'_, '_>,
    mut ev_select: MessageWriter<SelectSkill>,
    mut ev_commit: MessageWriter<CommitAction>,
    mut ev_reaction: MessageWriter<ReactionInput>,
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
            ui.heading(format!("Virtual Time {:.2}s", time.elapsed().as_secs_f64()));
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

            // 技能选择（选中高亮跟随键盘 Tab 游标）+ 提交
            ui.label("Skills (Tab):");
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
                    let btn =
                        ui.add_enabled(can_afford, egui::Button::new(label).selected(selected));
                    if btn.clicked() {
                        ev_select.write(SelectSkill(skill));
                    }
                }
            });
            if ui.button("✅ Commit (Space)").clicked() {
                ev_commit.write(CommitAction);
            }

            // 实时反应（前摇窗口内有效，校验在消费端）
            ui.separator();
            ui.label("Reactions (during windup):");
            ui.horizontal_wrapped(|ui| {
                let can_cancel = player_sta.unwrap_or(0) >= 2;
                if ui
                    .add_enabled(can_cancel, egui::Button::new("翻滚取消 Q (-2)"))
                    .clicked()
                {
                    ev_reaction.write(ReactionInput {
                        kind: ReactionKind::RollCancel,
                    });
                }
                let can_parry = player_sta.unwrap_or(0) >= 1;
                if ui
                    .add_enabled(can_parry, egui::Button::new("招架 E (-1)"))
                    .clicked()
                {
                    ev_reaction.write(ReactionInput {
                        kind: ReactionKind::Parry,
                    });
                }
            });

            ui.separator();
            ui.small(format!("Grid {}×{}", GRID_SIZE, GRID_SIZE));
            if ui.button("🔄 Reset Battle (also R)").clicked() {
                ev_reset.write(ResetBattle);
            }
        });
}
