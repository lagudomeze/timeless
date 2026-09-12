//! HUD：是否在等玩家决策 / 双方状态 / 待执行行动 / 战斗日志 / 操作提示。
//!
//! 纯表现：只**读**游戏状态（时间线、单位、日志），不写任何规则数据。
//!
//! **字体**：战斗中会出现的 CJK 文本（战斗日志正文、`单位` 兜底标签）必须能渲染，
//! 而 Bevy 默认字体不含 CJK，因此 HUD 显式指定 [`HUD_FONT`]。
//! HUD 自己的文案仍然是英文（AGENTS.md 的约定）；中文只出现在战斗日志里。
//! 字体覆盖由 `assets::font_covers_every_glyph_the_battle_log_can_emit` 这个测试守着。

use bevy::prelude::*;

use crate::ai::Intent;
use crate::combat::defense::{Dodging, Parrying, Stamina};
use crate::combat::skills::{FireballAction, MeleeAction, MenuSelection, SKILLS, skill_line};
use crate::combat::{Faction, Health};
use crate::movement::{Cell, JumpAction, Jumping, MoveAction, RollAction};
use crate::timeline::{Pending, Ready, ScheduledAction, Timeline};

use super::BattleLog;
use super::components::{HudLog, HudStatus};

/// 战斗日志在 HUD 上显示的行数。
const LOG_LINES: usize = 7;

/// HUD 用的字体资产路径。
///
/// 选 Noto Sans SC：OFL-1.1、简体覆盖全，且能同时渲染英文与 CJK
/// （因此不需要字体回退链）。资产与许可见 `assets/LICENSES.md`。
pub const HUD_FONT: &str = "fonts/NotoSansSC-Regular.otf";

/// HUD 里一行单位信息（从查询结果里摘出来的快照）。
struct UnitRow {
    entity: Entity,
    position: Vec3,
    cell: Cell,
    current: f32,
    max: f32,
    stamina: Option<(u32, u32)>,
    ready: bool,
    dodging: bool,
    parrying: bool,
    intent: Option<Intent>,
    airborne: bool,
}

/// 建立 HUD 节点（Startup 一次）。
///
/// 两段文本共用同一份字体句柄：Bevy 会为「字体句柄 + 字号」的组合各建一张字形图集，
/// 因此两种字号本来就是两张图集，句柄复用不会额外增加开销。
pub fn setup_hud(mut commands: Commands, assets: Res<AssetServer>) {
    let font: Handle<Font> = assets.load(HUD_FONT);
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
                TextFont::from_font_size(14.0).with_font(font.clone()),
                TextColor(Color::srgb(0.65, 0.78, 0.95)),
                HudStatus,
            ),
            (
                Text::new(""),
                TextFont::from_font_size(12.0).with_font(font),
                TextColor(Color::srgb(0.78, 0.82, 0.88)),
                HudLog,
            ),
        ],
    ));
}

/// 每帧刷新 HUD 文本。
///
/// 三个「只看有没有这个标记」的查询（`Ready` / `Dodging` / `Parrying`）合成一个
/// [`ParamSet`]：Bevy 的系统元组最多 16 个参数，而本系统要读的状态确实很多。
#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub fn update_hud_system(
    timeline: Res<Timeline>,
    log: Res<BattleLog>,
    menu: Res<MenuSelection>,
    units: Query<(Entity, &Faction, &Health, &Transform, &Cell)>,
    stamina_q: Query<&Stamina>,
    mut flags: ParamSet<(
        Query<(), With<Ready>>,
        Query<(), With<Dodging>>,
        Query<(), With<Parrying>>,
    )>,
    intents: Query<&Intent>,
    airborne: Query<&Jumping>,
    pending: Query<(Entity, &ScheduledAction), With<Pending>>,
    movements: Query<&MoveAction>,
    jumps: Query<&JumpAction>,
    fireballs: Query<&FireballAction>,
    melees: Query<&MeleeAction>,
    rolls: Query<&RollAction>,
    mut status_text: Query<&mut Text, (With<HudStatus>, Without<HudLog>)>,
    mut log_text: Query<&mut Text, (With<HudLog>, Without<HudStatus>)>,
) {
    let mut player: Option<UnitRow> = None;
    let mut enemy: Option<UnitRow> = None;
    for (entity, faction, health, transform, cell) in &units {
        let row = UnitRow {
            entity,
            position: transform.translation,
            cell: *cell,
            current: health.current,
            max: health.max,
            stamina: stamina_q.get(entity).ok().map(|s| (s.current, s.max)),
            ready: flags.p0().get(entity).is_ok(),
            dodging: flags.p1().get(entity).is_ok(),
            parrying: flags.p2().get(entity).is_ok(),
            intent: intents.get(entity).ok().copied(),
            airborne: airborne.get(entity).is_ok(),
        };
        match faction {
            Faction::Player => {
                player = Some(row);
            }
            Faction::Enemy => {
                enemy = Some(row);
            }
        }
    }

    // 文本只用 ASCII：Bevy 默认字体没有 · / — / 中文这些字形
    let state = if timeline.waiting_for_input() {
        if timeline.has_draft() {
            "WAITING FOR INPUT  (draft ready, Enter to commit)"
        } else {
            "WAITING FOR INPUT  (frozen)"
        }
    } else {
        "RUNNING"
    };

    let mut lines = vec![format!("TIMELINE   {state}"), String::new()];
    match player {
        Some(ref row) => {
            let action = pending_label(
                row.entity, &pending, &movements, &jumps, &fireballs, &melees, &rolls,
            );
            let air = if row.airborne { " (air)" } else { "" };
            let defense = defense_label(row);
            let stamina = row
                .stamina
                .map(|(current, max)| format!("{current}/{max}"))
                .unwrap_or_else(|| "-".to_string());
            lines.push(format!(
                "PLAYER   HP {:>3.0}/{:<3.0}   EN {:>3}   cell ({:>3},{:>3})   {defense}   act: {action}{air}",
                row.current, row.max, stamina, row.cell.x, row.cell.z
            ));
        }
        None => lines.push("PLAYER   down (press R to reset)".to_string()),
    }
    match enemy {
        Some(row) => {
            let distance = player
                .as_ref()
                .map(|player| {
                    let delta = row.position - player.position;
                    Vec2::new(delta.x, delta.z).length()
                })
                .unwrap_or_default();
            let action = pending_label(
                row.entity, &pending, &movements, &jumps, &fireballs, &melees, &rolls,
            );
            let intent = row.intent.map(intent_label).unwrap_or("no-brain");
            let air = if row.airborne { " (air)" } else { "" };
            lines.push(format!(
                "ENEMY    HP {:>3.0}/{:<3.0}   cell ({:>3},{:>3})   {:<8}   act: {action}{air}",
                row.current,
                row.max,
                row.cell.x,
                row.cell.z,
                defense_label(&row)
            ));
            lines.push(format!("         dist {distance:>4.1}   intent {intent}"));
        }
        None => lines.push("ENEMY    none".to_string()),
    }
    lines.push(String::new());
    // 技能菜单：只列精力负担得起的（其余打上 `x` 表示当前用不出来）
    let stamina_current = player.as_ref().and_then(|row| row.stamina).map(|(c, _)| c);
    let skills: Vec<String> = SKILLS
        .iter()
        .enumerate()
        .map(|(index, def)| {
            let affordable = stamina_current.is_some_and(|current| def.cost <= current);
            let line = skill_line(index, menu.index());
            if affordable {
                line
            } else {
                format!("x{}", &line[1..])
            }
        })
        .collect();
    lines.push(format!("skills {}", skills.join("  ")));
    lines.push(
        "keys   WASD move | Q fireball | E melee | Space jump | F roll | V parry".to_string(),
    );
    lines.push("       1-4 pick | Tab cycle | G use | R reset".to_string());
    lines.push("       F1 enter-to-commit | middle-drag pan".to_string());

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

/// 敌人意图的可读标签。
fn intent_label(intent: Intent) -> &'static str {
    match intent {
        Intent::Idle => "idle",
        Intent::Approach => "approach",
        Intent::Melee => "melee",
        Intent::Shoot => "shoot",
        Intent::Retreat => "retreat",
        Intent::Dodge => "dodge",
    }
}

/// 单位的当前状态：防御标记优先于「就绪 / 后摇」。
fn defense_label(row: &UnitRow) -> &'static str {
    if row.dodging {
        "dodging"
    } else if row.parrying {
        "parrying"
    } else if row.ready {
        "ready"
    } else {
        "busy"
    }
}

/// 该单位当前待执行的行动（没有就是 `-`）。
fn pending_label(
    actor: Entity,
    pending: &Query<(Entity, &ScheduledAction), With<Pending>>,
    movements: &Query<&MoveAction>,
    jumps: &Query<&JumpAction>,
    fireballs: &Query<&FireballAction>,
    melees: &Query<&MeleeAction>,
    rolls: &Query<&RollAction>,
) -> &'static str {
    let Some((entity, _)) = pending.iter().find(|(_, action)| action.actor == actor) else {
        return "-";
    };
    if movements.get(entity).is_ok() {
        return "move";
    }
    if jumps.get(entity).is_ok() {
        return "jump";
    }
    if fireballs.get(entity).is_ok() {
        return "fireball";
    }
    if melees.get(entity).is_ok() {
        return "melee";
    }
    if rolls.get(entity).is_ok() {
        return "roll";
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
    fn hud_reports_waiting_state_and_units() {
        let mut app = headless_app();
        app.update(); // Startup：组装单位 + 建 HUD
        app.update(); // 刷新文本

        let text = text_of(&mut app);
        assert!(
            text.contains("WAITING FOR INPUT"),
            "玩家就绪时世界应当停下等他：{text}"
        );
        assert!(text.contains("PLAYER"), "应当显示玩家信息：{text}");
        assert!(text.contains("ENEMY"), "应当显示敌人信息：{text}");
        assert!(text.contains("middle-drag"), "应当提示中键平移：{text}");
    }

    #[test]
    fn hud_shows_the_pending_action_after_input() {
        let mut app = headless_app();
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyQ);
        app.update(); // 声明火球 → 进入待执行
        app.update(); // 刷新文本

        let text = text_of(&mut app);
        assert!(text.contains("fireball"), "应当显示待执行行动：{text}");
    }

    /// HUD 技能菜单：列出注册表里的技能、标出当前选择、并标出负担不起的。
    #[test]
    fn hud_lists_skills_and_marks_the_selection() {
        let mut app = headless_app();
        app.update();
        app.update();

        let text = text_of(&mut app);
        assert!(text.contains("skills"), "应当有技能行：{text}");
        assert!(text.contains("attack"), "技能行应当列出攻击：{text}");
        assert!(
            text.contains(">1:attack"),
            "默认选中第一项应当被打上 `>`：{text}"
        );
        assert!(text.contains("Tab cycle"), "应当提示 Tab 循环技能：{text}");
    }
}
