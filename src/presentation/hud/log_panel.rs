//! 右下偏上的战斗日志：半透明、点标题折叠。
//!
//! 日志数据仍是 [`BattleLog`]（中文正文，见 [`crate::presentation::log`]）；
//! 这里只负责显示最后 [`LOG_LINES`] 行，并把折叠状态记在面板实体上。

use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};

use super::super::BattleLog;
use super::{HudCache, PANEL_BG, hud_text_tinted};

/// 面板宽度（像素）。
pub const LOG_PANEL_WIDTH: f32 = 380.0;
/// 显示的行数。
pub const LOG_LINES: usize = 8;

/// 日志面板根（挂着折叠状态）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct LogPanel;

/// 折叠状态：`true` = 只留标题条。
#[derive(Component, Reflect, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct LogCollapsed(pub bool);

/// 标题按钮（点击折叠 / 展开）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct LogHeaderButton;

/// 标题文本（显示 `[-]` / `[+]`）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct LogHeaderLabel;

/// 正文容器（折叠时 `Display::None`）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct LogBody;

/// 正文文本。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct LogBodyText;

/// 日志面板快照：正文与折叠状态都没变就整帧不碰 UI。
#[derive(Debug, Default, Clone, PartialEq)]
pub struct LogCache {
    collapsed: bool,
    body: String,
}

/// 日志面板：标题条（按钮）+ 正文。
pub fn log_panel(font: &Handle<Font>) -> impl Bundle {
    (
        Name::new("CombatLog"),
        LogPanel,
        LogCollapsed(false),
        // 吃掉指针：日志面板（含标题条）不接受世界点击
        FocusPolicy::Block,
        RelativeCursorPosition::default(),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(14.0),
            bottom: Val::Px(126.0),
            width: Val::Px(LOG_PANEL_WIDTH),
            flex_direction: FlexDirection::Column,
            border_radius: BorderRadius::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(PANEL_BG),
        children![
            (
                Name::new("CombatLogHeader"),
                Button,
                LogHeaderButton,
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.10, 0.12, 0.17, 0.9)),
                children![(
                    Name::new("CombatLogHeaderLabel"),
                    hud_text_tinted(font, 12.0, "COMBAT LOG  [-]", Color::srgb(0.78, 0.84, 0.92)),
                    LogHeaderLabel,
                )],
            ),
            (
                Name::new("CombatLogBody"),
                LogBody,
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                children![(
                    Name::new("CombatLogBodyText"),
                    hud_text_tinted(font, 12.0, "", Color::srgb(0.80, 0.84, 0.90)),
                    LogBodyText,
                )],
            ),
        ],
    )
}

/// 点标题条折叠 / 展开（`Changed<Interaction>` 保证一次点击只翻一次）。
/// 标题按钮查询：只在交互变化的那一帧处理。
type HeaderQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static ChildOf),
    (Changed<Interaction>, With<LogHeaderButton>),
>;

/// 点标题条折叠 / 展开（`Changed<Interaction>` 保证一次点击只翻一次）。
pub fn toggle_log_system(headers: HeaderQuery<'_, '_>, mut panels: Query<&mut LogCollapsed>) {
    for (interaction, child_of) in &headers {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Ok(mut collapsed) = panels.get_mut(child_of.parent()) {
            collapsed.0 = !collapsed.0;
        }
    }
}

/// 刷新正文与折叠外观。
pub fn update_log_panel_system(
    log: Res<BattleLog>,
    panels: Query<&LogCollapsed>,
    mut cache: ResMut<HudCache>,
    mut bodies: Query<&mut Node, With<LogBody>>,
    mut header_labels: Query<&mut Text, (With<LogHeaderLabel>, Without<LogBodyText>)>,
    mut body_texts: Query<&mut Text, (With<LogBodyText>, Without<LogHeaderLabel>)>,
) {
    let collapsed = panels
        .iter()
        .next()
        .map(|collapsed| collapsed.0)
        .unwrap_or(false);

    // 正文只在**真的有新日志**时重建：`Res::is_changed()` 是 Bevy 的资源变化检测，
    // 日志靠 `ResMut<BattleLog>` 追加，因此这里能精确命中「有新条目」那一帧。
    let body = if log.is_changed() || cache.log.body.is_empty() {
        let entries: Vec<&str> = log.entries().collect();
        let tail = entries.len().saturating_sub(LOG_LINES);
        let joined = entries[tail..].join("\n");
        if joined.is_empty() {
            "(nothing yet)".to_string()
        } else {
            joined
        }
    } else {
        cache.log.body.clone()
    };

    // 快照比对：正文与折叠状态都没变就整帧不碰 UI
    if cache.log.collapsed == collapsed && cache.log.body == body {
        return;
    }
    cache.log.collapsed = collapsed;
    cache.log.body.clone_from(&body);

    for mut text in &mut body_texts {
        **text = body.clone();
    }
    for mut node in &mut bodies {
        node.display = if collapsed {
            Display::None
        } else {
            Display::Flex
        };
    }
    for mut label in &mut header_labels {
        **label = if collapsed {
            "COMBAT LOG  [+]".to_string()
        } else {
            "COMBAT LOG  [-]".to_string()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_panel(app: &mut App) -> (Entity, Entity, Entity) {
        let body = app.world_mut().spawn((LogBody, Node::default())).id();
        let header = app
            .world_mut()
            .spawn((LogHeaderButton, Interaction::default(), Node::default()))
            .id();
        let panel = app
            .world_mut()
            .spawn((LogPanel, LogCollapsed(false), Node::default()))
            .id();
        app.world_mut()
            .entity_mut(panel)
            .add_children(&[header, body]);
        (panel, header, body)
    }

    /// 点击标题 → 折叠状态翻转；正文与标题记号跟着变。
    #[test]
    fn clicking_the_header_collapses_the_log() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<BattleLog>()
            .init_resource::<HudCache>()
            .add_systems(Update, (toggle_log_system, update_log_panel_system).chain());
        let (panel, header, body) = build_panel(&mut app);

        app.update();
        assert!(!app.world().get::<LogCollapsed>(panel).unwrap().0);
        assert_eq!(
            app.world().get::<Node>(body).unwrap().display,
            Display::Flex
        );

        *app.world_mut().get_mut::<Interaction>(header).unwrap() = Interaction::Pressed;
        app.update();
        assert!(app.world().get::<LogCollapsed>(panel).unwrap().0);
        assert_eq!(
            app.world().get::<Node>(body).unwrap().display,
            Display::None,
            "折叠后正文应当隐藏"
        );

        // 再点一次展开
        *app.world_mut().get_mut::<Interaction>(header).unwrap() = Interaction::None;
        app.update();
        *app.world_mut().get_mut::<Interaction>(header).unwrap() = Interaction::Pressed;
        app.update();
        assert!(!app.world().get::<LogCollapsed>(panel).unwrap().0);
    }

    /// 日志正文只显示最后 `LOG_LINES` 行。
    #[test]
    fn body_shows_only_the_tail() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<BattleLog>()
            .init_resource::<HudCache>()
            .add_systems(Update, update_log_panel_system);
        let _ = build_panel(&mut app);
        let text_entity = app.world_mut().spawn((LogBodyText, Text::new(""))).id();
        for index in 0..(LOG_LINES + 3) {
            app.world_mut()
                .resource_mut::<BattleLog>()
                .push(format!("line {index}"));
        }

        app.update();

        let shown = app.world().get::<Text>(text_entity).unwrap().0.clone();
        assert_eq!(shown.lines().count(), LOG_LINES);
        assert!(
            shown.contains(&format!("line {}", LOG_LINES + 2)),
            "{shown}"
        );
        assert!(!shown.contains("line 0\n"), "最早的几行应当被截掉：{shown}");
    }

    /// 日志没新增时正文不该被重写（`Res::is_changed()` 命中的那一帧才重建）。
    #[test]
    fn log_body_is_left_alone_when_the_log_has_not_changed() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<BattleLog>()
            .init_resource::<HudCache>()
            .add_systems(Update, update_log_panel_system);
        let _ = build_panel(&mut app);
        let text_entity = app.world_mut().spawn((LogBodyText, Text::new(""))).id();

        app.update();
        assert_eq!(
            app.world().get::<Text>(text_entity).unwrap().0,
            "(nothing yet)"
        );

        app.world_mut().get_mut::<Text>(text_entity).unwrap().0 = "SENTINEL".to_string();
        app.update();
        assert_eq!(
            app.world().get::<Text>(text_entity).unwrap().0,
            "SENTINEL",
            "没有新日志就不该重建正文"
        );

        app.world_mut()
            .resource_mut::<BattleLog>()
            .push("玩家 受到 10 点物理伤害");
        app.update();
        assert!(
            app.world()
                .get::<Text>(text_entity)
                .unwrap()
                .0
                .contains("物理伤害"),
            "有了新日志就必须刷新"
        );
    }
}
