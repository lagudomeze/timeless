//! # HUD：屏幕底部状态文本
//!
//! 回合 / 阶段状态、双方数值（生命 / 精力 / 当前意图）与按键提示。

use bevy::prelude::*;

use crate::combat::{
    AttackIntent, BattleLog, DodgeActive, Enemy, Health, Parry, Player, Stamina, intent_label,
};
use crate::menu::{DECISION_OPTIONS, MenuSelection, REACTION_OPTIONS, ReactionChoice};
use crate::movement::{FireballCast, MoveIntent, Position, RetreatIntent};
use crate::timeline::{TimeLineState, TurnPhase};

/// HUD 文本标记（屏幕底部）
#[derive(Component, Debug, Clone, Copy)]
pub struct HudText;

/// 战斗日志文本标记（屏幕右下角，最新消息在底部）
#[derive(Component, Debug, Clone, Copy)]
pub struct BattleLogText;

/// HUD 玩家行查询（数值 + 意图组合）
type PlayerHudQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Health,
        &'static Stamina,
        &'static Position,
        Option<&'static AttackIntent>,
        Option<&'static MoveIntent>,
        Option<&'static RetreatIntent>,
        Option<&'static DodgeActive>,
        Option<&'static Parry>,
        Option<&'static FireballCast>,
    ),
    With<Player>,
>;

/// HUD 敌人行查询（数值 + 意图组合）
type EnemyHudQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Health,
        &'static Position,
        Option<&'static AttackIntent>,
        Option<&'static MoveIntent>,
        Option<&'static RetreatIntent>,
    ),
    With<Enemy>,
>;

/// 创建 HUD 文本节点（setup 调用一次）
pub fn spawn_hud(commands: &mut Commands, asset_server: &AssetServer) {
    let font = asset_server.load::<Font>("fonts/NotoSansSC-Regular.otf");
    commands.spawn((
        HudText,
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
        Text::new(String::new()),
        TextFont {
            font: FontSource::Handle(font),
            font_size: FontSize::Px(18.0),
            ..default()
        },
        // NoWrap：HUD 用显式 \n 换行，绕开 ICU4X 缺失的 CJK 分词模型（避免报错刷屏）
        TextLayout {
            linebreak: LineBreak::NoWrap,
            justify: Justify::Left,
        },
        TextColor(Color::WHITE),
    ));
}

/// 创建战斗日志 UI 文本（setup 调用一次）
pub fn spawn_battle_log(commands: &mut Commands, asset_server: &AssetServer) {
    let font = asset_server.load::<Font>("fonts/NotoSansSC-Regular.otf");
    commands.spawn((
        BattleLogText,
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(12.0),
            bottom: Val::Px(12.0),
            ..default()
        },
        Text::new(String::new()),
        TextFont {
            font: FontSource::Handle(font),
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextLayout {
            linebreak: LineBreak::NoWrap,
            justify: Justify::Left,
        },
        TextColor(Color::srgb(0.85, 0.9, 0.95)),
    ));
}

/// 战斗日志：把 `BattleLog` 资源渲染成屏幕右下角多行文本（内容变化时才写回）
pub fn battle_log_system(log: Res<BattleLog>, mut text_q: Query<&mut Text, With<BattleLogText>>) {
    let Ok(mut text) = text_q.single_mut() else {
        return;
    };
    // 按时间顺序排列：底部锚定节点向上生长，最新一条自然落在最底部
    let content = log.entries.iter().cloned().collect::<Vec<_>>().join("\n");
    if text.0.as_str() != content {
        **text = content;
    }
}

/// HUD：回合状态 + 双方数值 + 按键提示
pub fn hud_system(
    tl: Res<TimeLineState>,
    menu: Res<MenuSelection>,
    mut hud_q: Query<&mut Text, With<HudText>>,
    player_q: PlayerHudQuery<'_, '_>,
    enemy_q: EnemyHudQuery<'_, '_>,
) {
    let Ok(mut text) = hud_q.single_mut() else {
        return;
    };

    let (state, keys_hint, sel_line) = match tl.phase {
        TurnPhase::Decision => {
            let mut sel = String::new();
            for (i, a) in DECISION_OPTIONS.iter().enumerate() {
                sel.push_str(if i == menu.index { " ▶ " } else { "   " });
                sel.push_str(match a {
                    crate::menu::Action::Attack => "攻击",
                    crate::menu::Action::Move => "移动",
                    crate::menu::Action::Roll => "翻滚",
                    crate::menu::Action::Fireball => "火球（-2 精力）",
                    _ => "?",
                });
                sel.push('\n');
            }
            (
                "决策暂停：选择行动，空格提交".to_string(),
                "Tab 切换技能 | WASD/方向键 移动 | 空格 提交 | R 重置 | 右键拖动 视角 | 滚轮 缩放"
                    .to_string(),
                sel,
            )
        }
        TurnPhase::Reaction => {
            let mut sel = String::new();
            for (i, c) in REACTION_OPTIONS.iter().enumerate() {
                sel.push_str(if i == menu.index { " ▶ " } else { "   " });
                sel.push_str(match c {
                    ReactionChoice::Continue => "继续攻击",
                    ReactionChoice::RollCancel => "翻滚取消（-2 精力）",
                    ReactionChoice::Parry => "招架（-1 精力）",
                });
                sel.push('\n');
            }
            (
                "反应暂停：敌人也在攻击你！".to_string(),
                "Tab 选择 | 空格 执行 | Q 翻滚取消 | 右键拖动 视角 | 滚轮 缩放".to_string(),
                sel,
            )
        }
        TurnPhase::Resolving => ("结算中...".to_string(), String::new(), String::new()),
        TurnPhase::GameOver => (
            "战斗结束".to_string(),
            "R 重置战斗 | 右键拖动 视角 | 滚轮 缩放".to_string(),
            String::new(),
        ),
    };

    let p_line = match player_q.single() {
        Ok((h, s, p, attack, mov, retreat, dodge, parry, fireball)) => {
            let action = intent_label(attack, mov, retreat, parry, fireball);
            let dodge_tag = if dodge.is_some() { "（闪避）" } else { "" };
            format!(
                "玩家  生命 {}/{}  精力 {}/{}  行动 {}{dodge_tag}  @({},{})",
                h.current, h.max, s.current, s.max, action, p.0.x, p.0.y
            )
        }
        Err(_) => "玩家  已阵亡".to_string(),
    };
    let e_line = match enemy_q.single() {
        Ok((h, p, attack, mov, retreat)) => format!(
            "敌人  生命 {}/{}  行动 {}  @({},{})",
            h.current,
            h.max,
            intent_label(attack, mov, retreat, None, None),
            p.0.x,
            p.0.y
        ),
        Err(_) => "敌人  已被击败".to_string(),
    };

    **text = format!(
        "[回合 {}] {}\n{sel_line}{}\n{}\n{}",
        tl.global_tick, state, p_line, e_line, keys_hint
    );
}
