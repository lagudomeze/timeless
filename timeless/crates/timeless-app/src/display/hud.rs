//! # HUD：屏幕底部状态文本
//!
//! 虚拟时间、双方数值（生命 / 精力 / 当前意图）、技能选择与按键提示。

use bevy::prelude::*;
use bevy::time::Virtual;

use crate::combat::{ActionLabelQuery, BattleLog, Enemy, Health, Player, Stamina, action_label};
use crate::menu::{
    CanAttack, CanFireball, CanMove, CanRoll, MenuSelection, SKILLS, available_skills,
};
use crate::movement::Position;

/// HUD 文本标记（屏幕底部）
#[derive(Component, Debug, Clone, Copy)]
pub struct HudText;

/// 战斗日志文本标记（屏幕右下角，最新消息在底部）
#[derive(Component, Debug, Clone, Copy)]
pub struct BattleLogText;

/// HUD 玩家行查询（数值 + 位置）
type PlayerHudQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Health, &'static Stamina, &'static Position), With<Player>>;

/// HUD 敌人行查询（数值 + 位置）
type EnemyHudQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Health, &'static Position), With<Enemy>>;

/// HUD 能力查询（技能选项展示）
type HudCapabilityQuery<'w, 's> = Query<
    'w,
    's,
    (Has<CanAttack>, Has<CanMove>, Has<CanRoll>, Has<CanFireball>),
    (With<Player>, Without<Enemy>),
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

/// HUD：虚拟时间 + 双方数值 + 技能选择 + 按键提示
pub fn hud_system(
    time: Res<Time<Virtual>>,
    menu: Res<MenuSelection>,
    mut hud_q: Query<&mut Text, With<HudText>>,
    player_q: PlayerHudQuery<'_, '_>,
    enemy_q: EnemyHudQuery<'_, '_>,
    capability_q: HudCapabilityQuery<'_, '_>,
    actions: ActionLabelQuery<'_, '_>,
) {
    let Ok(mut text) = hud_q.single_mut() else {
        return;
    };

    // 技能选择列表（Tab / 面板选择，随时可改）
    let available = capability_q
        .single()
        .ok()
        .map(|(a, m, r, f)| available_skills(a, m, r, f))
        .unwrap_or_default();
    let mut sel = String::new();
    for (i, &skill) in available.iter().enumerate() {
        sel.push_str(if i == menu.index { " ▶ " } else { "   " });
        let def = &SKILLS[skill];
        sel.push_str(def.label);
        if def.cost > 0 {
            sel.push_str(&format!("（-{} 精力）", def.cost));
        }
        sel.push('\n');
    }

    let p_line = match player_q.single() {
        Ok((entity, h, s, p)) => {
            let action = action_label(entity, &actions);
            format!(
                "玩家  生命 {}/{}  精力 {}/{}  行动 {}  @({},{})",
                h.current, h.max, s.current, s.max, action, p.0.x, p.0.y
            )
        }
        Err(_) => "玩家  已阵亡".to_string(),
    };
    let e_line = match enemy_q.single() {
        Ok((entity, h, p)) => {
            let action = action_label(entity, &actions);
            format!(
                "敌人  生命 {}/{}  行动 {}  @({},{})",
                h.current, h.max, action, p.0.x, p.0.y
            )
        }
        Err(_) => "敌人  已被击败".to_string(),
    };

    **text = format!(
        "时间 {:.1}s\n{sel}{}\n{}\n{}",
        time.elapsed().as_secs_f64(),
        p_line,
        e_line,
        "Tab 切换技能 | WASD/方向键 移动 | 空格 提交 | Q 翻滚取消 | E 招架 | R 重置 | 右键拖动 视角 | 滚轮 缩放"
    );
}
