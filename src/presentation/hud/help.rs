//! 帮助面板：`F1` 开合。常驻的按键提示从屏幕上撤掉了，改到这里。
//!
//! 输入域只把 `F1` 翻译成 [`ToggleHelp`] 消息（见 [`crate::input`]），
//! 由本系统消费——「UI 输入只翻译、不执行」。

use bevy::prelude::*;

use super::hud_text;

/// 帮助面板宽度（像素）。
pub const HELP_WIDTH: f32 = 420.0;

/// 切换帮助面板（写：`input` 的 `F1`；消费：[`toggle_help_system`]）。
#[derive(Message, Debug, Clone, Copy)]
pub struct ToggleHelp;

/// 帮助面板根标记。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct HelpPanel;

/// 面板里列出的按键（英文，与 `AGENTS.md` 的键位表保持一致）。
pub const HELP_LINES: &[&str] = &[
    "KEYBOARD",
    "  arrows          move one cell (screen-relative)",
    "  1-4             use skill slot 1-4 directly",
    "  Q / W / E / R   hotkeys: fireball / melee / roll / parry",
    "  Tab / Shift+Tab cycle affordable skills",
    "  G               use selected skill",
    "  C               jump (cannot be interrupted)",
    "  Space           pause / resume",
    "  Shift + a key   spend 1 Focus: no windup on that action",
    "  F5              reset the battle",
    "  F1              close this help",
    "MOUSE",
    "  middle drag     pan the camera",
    "  left click      move to that cell (or use the selected skill on a unit)",
    "  right click     cancel the pending action (stop)",
    "  wheel           zoom in / out",
    "HOW TO READ THE HUD",
    "  top bar         one lane per unit; the bar starts at \"now\"",
    "  blue / red      player / enemy (declared drafts are translucent)",
    "  line in a bar   when that action resolves (impact / landing)",
    "  right of bars   staged units: ready to decide, nothing declared yet",
    "  ground shadow   how high a unit is above the ground",
    "  EN badge        energy cost of the skill slot",
];

/// 帮助面板（默认隐藏）。
pub fn help_panel(font: &Handle<Font>) -> impl Bundle {
    (
        Name::new("HelpPanel"),
        HelpPanel,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(12.0),
            left: Val::Percent(50.0),
            margin: UiRect::new(Val::Px(-HELP_WIDTH / 2.0), Val::Auto, Val::Auto, Val::Auto),
            width: Val::Px(HELP_WIDTH),
            padding: UiRect::all(Val::Px(14.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            display: Display::None,
            border_radius: BorderRadius::all(Val::Px(10.0)),
            ..default()
        },
        // 帮助面板压在战场上方，底色要比常驻面板更实，文字才读得清
        BackgroundColor(Color::srgba(0.04, 0.05, 0.08, 0.94)),
        children![
            (
                Name::new("HelpTitle"),
                hud_text(font, 15.0, "HELP  (F1 to close)"),
            ),
            (Name::new("HelpSpacer"), hud_text(font, 11.0, ""),),
            (
                Name::new("HelpKeys"),
                hud_text(font, 12.0, HELP_LINES.join("\n")),
            ),
        ],
    )
}

/// `F1` 消息 → 显示 / 隐藏帮助面板。
pub fn toggle_help_system(
    mut requests: MessageReader<ToggleHelp>,
    mut panels: Query<&mut Node, With<HelpPanel>>,
) {
    if requests.read().next().is_none() {
        return;
    }
    for mut node in &mut panels {
        node.display = if node.display == Display::None {
            Display::Flex
        } else {
            Display::None
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_message_shows_and_hides_the_panel() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<ToggleHelp>()
            .add_systems(Update, toggle_help_system);
        let panel = app
            .world_mut()
            .spawn((
                HelpPanel,
                Node {
                    display: Display::None,
                    ..default()
                },
            ))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<Node>(panel).unwrap().display,
            Display::None,
            "默认是关的：常驻按键提示已经撤掉"
        );

        app.world_mut().write_message(ToggleHelp);
        app.update();
        assert_eq!(
            app.world().get::<Node>(panel).unwrap().display,
            Display::Flex
        );

        app.world_mut().write_message(ToggleHelp);
        app.update();
        assert_eq!(
            app.world().get::<Node>(panel).unwrap().display,
            Display::None
        );
    }
}
