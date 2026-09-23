//! 「无法操作」提示：一条会自己淡出的短句（锚在玩家面板正上方）。
//!
//! 数据流：各声明系统在拒绝输入时写 [`ActionBlocked`]（消息定义在 `timeline`，
//! 因为那讲的是「谁可以决策」），本模块消费它。计时器走 **`Time<Real>`**——
//! 世界冻结（等玩家输入）时提示也必须能自己消失。
//!
//! 为什么不复用别的区域：技能 tooltip 在正下方居中、战斗日志在右下，都会和它抢位置；
//! 锚在玩家面板上方既空着、又和「玩家现在能不能动」这件事最近。

use bevy::prelude::*;

use crate::timeline::{ActionBlocked, BlockReason};
use crate::world::BlockRefused;

use super::hud_text_tinted;

/// 提示停留时长（真实秒）：够读完一句短语，又不至于糊在屏幕上。
pub const HINT_SECS: f32 = 2.0;

/// 提示条根节点（默认隐藏）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct ActionHint;

/// 提示正文。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct ActionHintText;

/// 提示剩余时间（真实秒）；`<= 0` 表示不显示。
#[derive(Resource, Debug, Default)]
pub struct HintTimer(pub f32);

/// 预演读数（写：`interaction` 的悬停系统；消费：[`update_action_hint_system`]）。
///
/// `None` = 没有预演目标（隐藏读数）；`Some(text)` = 显示"技能 · 距离 · 预计伤害"。
/// 和"无法操作"共用同一条提示条：被拒的输入优先级更高（它是即时反馈）。
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct PreviewReadout(pub Option<String>);

/// 提示条：玩家面板顶边（118px）再往上 8px，默认隐藏。
pub fn hint_panel(font: &Handle<Font>) -> impl Bundle {
    (
        Name::new("ActionHint"),
        ActionHint,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(14.0),
            bottom: Val::Px(126.0),
            display: Display::None,
            ..default()
        },
        children![(
            Name::new("ActionHintText"),
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.10, 0.12, 0.17, 0.92)),
            hud_text_tinted(font, 12.0, "", WARN_TEXT),
            ActionHintText,
        )],
    )
}

/// 消费 [`ActionBlocked`]：起计时器、写文案、显示；到点自己隐藏。
///
/// 也消费 [`BlockRefused`]（`world` 域自己的拒绝原因）：方块交互被拒时同样要走
/// 这条提示——`world` 是纯数据域，不认识时间线的 `ActionBlocked`，所以它的话由
/// 表现层翻译成同一条提示条上的文案。
pub fn update_action_hint_system(
    time: Res<Time<Real>>,
    mut timer: ResMut<HintTimer>,
    mut blocked: MessageReader<ActionBlocked>,
    mut refused: MessageReader<BlockRefused>,
    mut readouts: MessageReader<PreviewReadout>,
    mut nodes: Query<&mut Node, With<ActionHint>>,
    mut texts: Query<(&mut Text, &mut TextColor, &mut BackgroundColor), With<ActionHintText>>,
) {
    let refused_message = refused
        .read()
        .last()
        .map(|BlockRefused::TerrainNotLoaded| "NO GROUND HERE · chunk not loaded");
    let blocked_message = blocked.read().last().map(|last| match last.reason {
        BlockReason::Busy => "CAN'T ACT YET · still busy",
        BlockReason::NotEnoughEnergy => "NOT ENOUGH ENERGY",
    });
    if let Some(message) = refused_message.or(blocked_message) {
        timer.0 = HINT_SECS;
        for (mut text, mut color, mut background) in &mut texts {
            **text = message.to_string();
            *color = TextColor(WARN_TEXT);
            *background = BackgroundColor(WARN_BG);
        }
        for mut node in &mut nodes {
            node.display = Display::Flex;
        }
    } else if timer.0 <= 0.0 {
        // 没有"被拒"的提示时，提示条让给预演读数（`None` = 收起）
        if let Some(readout) = readouts.read().last() {
            match &readout.0 {
                Some(message) => {
                    for (mut text, mut color, mut background) in &mut texts {
                        **text = message.clone();
                        *color = TextColor(INFO_TEXT);
                        *background = BackgroundColor(INFO_BG);
                    }
                    for mut node in &mut nodes {
                        node.display = Display::Flex;
                    }
                }
                None => {
                    for mut node in &mut nodes {
                        node.display = Display::None;
                    }
                }
            }
        }
    }

    if timer.0 <= 0.0 {
        return;
    }
    timer.0 = (timer.0 - time.delta_secs()).max(0.0);
    if timer.0 <= 0.0 {
        for mut node in &mut nodes {
            node.display = Display::None;
        }
    }
}

/// 「无法操作」的文字 / 底色（偏暖，提醒性质）。
const WARN_TEXT: Color = Color::srgb(1.0, 0.88, 0.80);
/// 见 [`WARN_TEXT`]。
const WARN_BG: Color = Color::srgba(0.10, 0.12, 0.17, 0.92);
/// 预演读数的文字 / 底色（偏冷，信息性质）。
const INFO_TEXT: Color = Color::srgb(0.86, 0.92, 1.0);
/// 见 [`INFO_TEXT`]。
const INFO_BG: Color = Color::srgba(0.06, 0.08, 0.12, 0.92);

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    fn hint_app() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .init_resource::<HintTimer>()
            .add_message::<ActionBlocked>()
            .add_message::<BlockRefused>()
            .add_message::<PreviewReadout>()
            .add_systems(Update, update_action_hint_system);
        let node = app
            .world_mut()
            .spawn((
                ActionHint,
                Node {
                    display: Display::None,
                    ..default()
                },
            ))
            .id();
        let text = app.world_mut().spawn((ActionHintText, Text::new(""))).id();
        (app, node, text)
    }

    /// 被拒的输入要看得见，而且**在冻结的世界里也会自己消失**。
    #[test]
    fn a_blocked_input_shows_a_message_that_fades_out() {
        let (mut app, node, text) = hint_app();
        app.world_mut().write_message(ActionBlocked::BUSY);
        app.update();

        assert_eq!(
            app.world().get::<Node>(node).unwrap().display,
            Display::Flex,
            "被拒的输入应当让提示显示出来"
        );
        let shown = app.world().get::<Text>(text).unwrap().0.clone();
        assert!(
            shown.contains("CAN'T ACT"),
            "文案要说清楚是「还动不了」而不是别的：{shown}"
        );

        // 2.0s 之后自己消失（每帧 0.1s，第一帧 dt=0，所以跑够 30 帧）
        for _ in 0..30 {
            app.update();
        }
        assert_eq!(
            app.world().get::<Node>(node).unwrap().display,
            Display::None,
            "提示到点应当自己隐藏，不需要玩家做任何事"
        );
    }

    /// 方块交互被拒也走同一条提示条——`world` 只宣布原因，文案由表现层给。
    ///
    /// 这条钉的是"写了没人读"：`BlockRefused` 有生产者、没有消费者时，
    /// 玩家在没加载的地方按 B/V **什么都看不到**，只能在代码里读到它。
    #[test]
    fn a_refused_block_edit_reaches_the_same_hint_bar() {
        let (mut app, node, text) = hint_app();
        app.world_mut()
            .write_message(BlockRefused::TerrainNotLoaded);
        app.update();

        assert_eq!(
            app.world().get::<Node>(node).unwrap().display,
            Display::Flex,
            "方块交互被拒也应当看得见"
        );
        let shown = app.world().get::<Text>(text).unwrap().0.clone();
        assert!(
            shown.contains("GROUND"),
            "文案要说清楚是「这里没有地」而不是别的：{shown}"
        );
    }

    /// 精力不足是另一种原因，文案不同。
    #[test]
    fn running_out_of_energy_gets_its_own_message() {
        let (mut app, node, text) = hint_app();
        app.world_mut().write_message(ActionBlocked::NO_ENERGY);
        app.update();

        assert_eq!(
            app.world().get::<Node>(node).unwrap().display,
            Display::Flex
        );
        assert!(
            app.world().get::<Text>(text).unwrap().0.contains("ENERGY"),
            "{}",
            app.world().get::<Text>(text).unwrap().0
        );
    }
}
