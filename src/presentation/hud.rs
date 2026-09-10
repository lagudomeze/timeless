//! HUD：阶段 / 轮次 / 双方状态 / 本轮声明 / 战斗日志 / 操作提示。
//!
//! 纯表现：只**读**游戏状态（时间线、单位、日志），不写任何规则数据。
//!
//! 文字一律用英文——Bevy 默认字体不含 CJK，中文界面需要自带字体资产
//! （见 TODO.md 的表现层待办）；控制台日志仍输出中文。

use bevy::prelude::*;

use crate::ai::AttackCooldown;
use crate::combat::skills::{MeleeAction, ShootAction};
use crate::combat::{Faction, Health};
use crate::movement::MoveAction;
use crate::timeline::{Declared, ScheduledAction, Timeline};

use super::BattleLog;
use super::components::{HudLog, HudStatus};

/// 战斗日志在 HUD 上显示的行数。
const LOG_LINES: usize = 7;

/// 建立 HUD 节点（Startup 一次）。
pub fn setup_hud(mut commands: Commands) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: px(10.0),
            left: px(10.0),
            padding: UiRect::all(px(10.0)),
            flex_direction: FlexDirection::Column,
            row_gap: px(6.0),
            max_width: px(640.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.62)),
        children![
            (
                Text::new(""),
                TextFont {
                    font_size: 14.0.into(),
                    ..default()
                },
                TextColor(Color::srgb(0.65, 0.78, 0.95)),
                HudStatus,
            ),
            (
                Text::new(""),
                TextFont {
                    font_size: 12.0.into(),
                    ..default()
                },
                TextColor(Color::srgb(0.78, 0.82, 0.88)),
                HudLog,
            ),
        ],
    ));
}

/// 每帧刷新 HUD 文本。
#[allow(clippy::too_many_arguments)]
pub fn update_hud_system(
    timeline: Res<Timeline>,
    log: Res<BattleLog>,
    units: Query<(
        Entity,
        &Faction,
        &Health,
        &Transform,
        Option<&AttackCooldown>,
    )>,
    declared: Query<(Entity, &ScheduledAction), With<Declared>>,
    movements: Query<&MoveAction>,
    shoots: Query<&ShootAction>,
    melees: Query<&MeleeAction>,
    mut status_text: Query<&mut Text, (With<HudStatus>, Without<HudLog>)>,
    mut log_text: Query<&mut Text, (With<HudLog>, Without<HudStatus>)>,
) {
    let mut player: Option<(Entity, Vec3, f32, f32)> = None;
    let mut enemy: Option<(Entity, Vec3, f32, f32, Option<&AttackCooldown>)> = None;
    for (entity, faction, health, transform, cooldown) in &units {
        match faction {
            Faction::Player => {
                player = Some((entity, transform.translation, health.current, health.max));
            }
            Faction::Enemy => {
                enemy = Some((
                    entity,
                    transform.translation,
                    health.current,
                    health.max,
                    cooldown,
                ));
            }
        }
    }

    // 文本只用 ASCII：Bevy 默认字体没有 · / — / 中文这些字形
    let phase = match timeline.phase() {
        crate::timeline::Phase::Planning => "PLANNING  (frozen, waiting for commit)".to_string(),
        crate::timeline::Phase::Resolving => match timeline.window_remaining() {
            Some(remaining) => format!("RESOLVING  ({remaining:.2}s left)"),
            None => "RESOLVING".to_string(),
        },
    };

    let mut lines = vec![
        format!("ROUND {:>2}   {phase}", timeline.round()),
        String::new(),
    ];
    match player {
        Some((entity, position, current, max)) => lines.push(format!(
            "PLAYER   HP {current:>3.0}/{max:<3.0}   ({:>5.1}, {:>5.1})   act: {}",
            position.x,
            position.z,
            declared_label(entity, &declared, &movements, &shoots, &melees)
        )),
        None => lines.push("PLAYER   down (press R to reset)".to_string()),
    }
    match enemy {
        Some((entity, position, current, max, cooldown)) => {
            let distance = player
                .map(|(_, player_position, _, _)| {
                    let delta = position - player_position;
                    Vec2::new(delta.x, delta.z).length()
                })
                .unwrap_or_default();
            let ready = match cooldown {
                Some(cooldown) if cooldown.is_ready() => "gun ready",
                Some(_) => "gun cooling",
                None => "no gun",
            };
            lines.push(format!(
                "ENEMY    HP {current:>3.0}/{max:<3.0}   ({:>5.1}, {:>5.1})   act: {}",
                position.x,
                position.z,
                declared_label(entity, &declared, &movements, &shoots, &melees)
            ));
            lines.push(format!("         dist {distance:>4.1}   {ready}"));
        }
        None => lines.push("ENEMY    none".to_string()),
    }
    lines.push(String::new());
    lines.push("keys   WASD move | Space shoot | E melee | Enter commit".to_string());
    lines.push("       R reset | middle-drag to pan the camera".to_string());

    for mut text in &mut status_text {
        **text = lines.join("\n");
    }

    let entries: Vec<&str> = log.entries().collect();
    let tail = entries.len().saturating_sub(LOG_LINES);
    let log_body = entries[tail..].join("\n");
    for mut text in &mut log_text {
        **text = if log_body.is_empty() {
            "log    (nothing yet)".to_string()
        } else {
            log_body.clone()
        };
    }
}

/// 本轮该单位声明了什么（没声明就是 `—`）。
fn declared_label(
    actor: Entity,
    declared: &Query<(Entity, &ScheduledAction), With<Declared>>,
    movements: &Query<&MoveAction>,
    shoots: &Query<&ShootAction>,
    melees: &Query<&MeleeAction>,
) -> &'static str {
    let Some((entity, _)) = declared.iter().find(|(_, action)| action.actor == actor) else {
        return "-";
    };
    if let Ok(movement) = movements.get(entity) {
        return if movement.axis == Vec2::ZERO {
            "hold"
        } else {
            "move"
        };
    }
    if shoots.get(entity).is_ok() {
        return "shoot";
    }
    if melees.get(entity).is_ok() {
        return "melee";
    }
    "action"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::headless_app;

    fn text_of(app: &mut App) -> String {
        let mut query = app.world_mut().query_filtered::<&Text, With<HudStatus>>();
        query
            .iter(app.world())
            .next()
            .expect("HUD 应当有状态文本")
            .0
            .clone()
    }

    #[test]
    fn hud_reports_phase_units_and_keys() {
        let mut app = headless_app();
        app.update(); // Startup：组装单位 + 建 HUD
        app.update(); // 刷新文本

        let text = text_of(&mut app);
        assert!(text.contains("PLANNING"), "应当显示规划阶段：{text}");
        assert!(text.contains("PLAYER"), "应当显示玩家信息：{text}");
        assert!(text.contains("ENEMY"), "应当显示敌人信息：{text}");
        assert!(
            text.contains("middle-drag to pan the camera"),
            "应当提示中键平移：{text}"
        );
    }

    #[test]
    fn hud_reports_the_declared_action() {
        let mut app = headless_app();
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.update(); // 声明射击
        app.update(); // 刷新文本

        let text = text_of(&mut app);
        assert!(text.contains("shoot"), "应当显示本轮声明：{text}");
    }
}
