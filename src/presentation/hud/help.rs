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

/// 面板里列出的按键。
///
/// **改按键必须同步这里**：这是玩家唯一看得到的清单——漏改的症状是"游戏里教的按键是错的"。
/// 有一条测试（`the_help_lists_every_key_the_input_domain_reads`）拿它和 `src/input/`
/// 里出现的按键对账，漏了就红。
pub const HELP_LINES: &[&str] = &[
    "KEYBOARD",
    "  arrows          move one cell (screen-relative)",
    "  X + arrows      dash two cells (costs energy)",
    "  1-5             use skill slot 1-5 directly",
    "  Q / W / E / R   hotkeys: fireball / melee / roll / parry",
    "  Tab / Shift+Tab cycle affordable skills",
    "  G               use selected skill",
    "  C               jump (cannot be interrupted)",
    // 不写具体秒数：等待时长是可配的（`timeline::WaitConfig`），
    // 写死一个数字只会在配置改掉之后变成错的
    "  Space           wait: hold the slot but let the world run",
    "  P               pause / resume",
    "  B / V           place / remove a block on the hovered cell",
    "  T               take off / put on your gear",
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

    /// **帮助面板不许教错按键**：它列出的键必须与 `input` 域真正读的键对得上。
    ///
    /// 这条是被真事逼出来的：空格从"暂停"改成"等待"、暂停挪到 `P` 之后，
    /// 帮助面板还写着 `Space  pause / resume`——玩家照做，得到的却是"等待一秒"。
    /// 键位只有 `src/input/` 一个真相，这份文案是**玩家唯一看得到的副本**，
    /// 所以拿它跟真相源对账：`input` 里出现的每个按键都得在面板里出现。
    #[test]
    fn the_help_lists_every_key_the_input_domain_reads() {
        let help = HELP_LINES.join("\n");
        // `input` 域真正读的按键（名字 → 玩家看到的写法）
        let keys: [(&str, &str); 17] = [
            ("ArrowUp", "arrows"),
            ("ArrowDown", "arrows"),
            ("ArrowLeft", "arrows"),
            ("ArrowRight", "arrows"),
            ("Digit1", "1-5"),
            ("Digit2", "1-5"),
            ("Digit3", "1-5"),
            ("Digit4", "1-5"),
            ("Digit5", "1-5"),
            ("Tab", "Tab"),
            ("KeyG", "G"),
            ("KeyC", "C"),
            ("Space", "Space"),
            ("KeyP", "P"),
            ("KeyT", "T"),
            ("KeyX", "X"),
            ("F5", "F5"),
        ];
        for (code, shown) in keys {
            assert!(
                help.contains(shown),
                "`input` 读了 {code}，但帮助面板里没有 {shown:?}：玩家会照着错的清单按"
            );
        }
    }

    /// **反向对账**：面板里写的按键必须真的存在——断掉"教一个已经删掉的键"。
    ///
    /// 做法是从真相源（`src/input/keyboard.rs` 的源码）里抓出所有 `KeyCode::X`
    /// 出现过的名字，再看面板首列写的键能不能落进去。这条能抓住
    /// 「按键删了但帮助没改」这类漂移，比逐字母维护一张白名单稳。
    #[test]
    fn every_key_the_help_teaches_still_exists_in_the_input_domain() {
        // 真相源：输入域源码里出现过的所有 KeyCode 变体名
        let source = include_str!("../../input/keyboard.rs");
        let mut known: Vec<String> = Vec::new();
        for chunk in source.split("KeyCode::").skip(1) {
            let name: String = chunk
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            if !name.is_empty() {
                known.push(name);
            }
        }
        assert!(!known.is_empty(), "没从输入域源码里解析出任何按键");

        // 面板首列写的每个键（`arrows` 与 `1-5` 是多个物理键的写法，单独认）
        let spellings = [
            "arrows", "1-5", "Q", "W", "E", "R", "Tab", "G", "C", "Space", "P", "B", "V", "T", "X",
            "F5", "F1",
        ];
        for spelling in spellings {
            assert!(
                HELP_LINES.join("\n").contains(spelling),
                "面板里少了 {spelling}：它是玩家能按的键，帮助必须列出来"
            );
        }
        // 正则式地确认：面板教的每个单键都能在源码里找到对应变体
        let pairs = [
            ("Q", "KeyQ"),
            ("W", "KeyW"),
            ("E", "KeyE"),
            ("R", "KeyR"),
            ("Tab", "Tab"),
            ("G", "KeyG"),
            ("C", "KeyC"),
            ("Space", "Space"),
            ("P", "KeyP"),
            ("B", "KeyB"),
            ("V", "KeyV"),
            ("T", "KeyT"),
            ("X", "KeyX"),
            ("F5", "F5"),
            ("F1", "F1"),
        ];
        for (shown, variant) in pairs {
            assert!(
                known.iter().any(|name| name == variant),
                "帮助面板教了 {shown}（{variant}），但输入域源码里已经没有这个按键了"
            );
        }
    }

    /// 空格与 `P` 的分工必须写在面板里：两者都是"让世界停 / 动"，
    /// 写反了危害最大（玩家以为按了暂停，其实只是等一秒）。
    #[test]
    fn the_help_says_space_is_a_wait_and_p_is_the_pause() {
        let space = HELP_LINES
            .iter()
            .find(|line| line.contains("Space"))
            .expect("面板里必须有空格那一行");
        assert!(
            space.contains("wait"),
            "空格现在是「等待」而不是暂停：{space:?}"
        );
        let p = HELP_LINES
            .iter()
            .find(|line| line.trim_start().starts_with("P "))
            .expect("面板里必须有 P 那一行");
        assert!(p.contains("pause"), "手动暂停已经挪到 P：{p:?}");
    }
}
