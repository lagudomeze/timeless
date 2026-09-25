//! 「指针是不是压在 UI 上」——交互域唯一的 UI 门控判据。
//!
//! 没有它，鼠标点面板会**同时**给世界下单（点日志标题会顺手把人走一格），
//! 光标离开战场后悬停读数与高亮也会继续留在世界里。
//!
//! ## 判据为什么是 `RelativeCursorPosition` 而不是 `Interaction`
//!
//! 两者都是 bevy_ui 的 `ui_focus_system`（`PreUpdate`，早于 `Update`）算出来的，
//! 但**只有前者与点击生命周期无关**：
//!
//! - `Interaction` 在按下那一刻会从 `Hovered` 变成 `Pressed`，而把它复位回
//!   `None` 要靠**松手事件**。实机踩到过：一次点到面板上的点击没收到松手，
//!   那个节点就一直挂着 `Pressed`——于是"指针在 UI 上"永远为真，**世界再也点不动**；
//! - `RelativeCursorPosition.cursor_over` 每帧按光标与节点的几何关系重算，
//!   与按没按下无关，因此既不会被卡住，也天然覆盖"按下时"（按下必然先在它上面）。
//!
//! 判据**不自建命中测试**，用的是 Bevy 已经算好的结果：HUD 的区域根节点声明
//! [`RelativeCursorPosition`]（以及 `FocusPolicy::Block`），见
//! [`crate::presentation::hud`] 的不变量。技能槽与日志标题是 `Button`，
//! 它们**在自己的区域根节点之内**，所以不需要单独声明。
//!
//! HUD 根节点不声明它——否则整屏都算"压在 UI 上"。

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

/// 本帧指针是否压在会吃点击的 HUD 区域上。
///
/// 注册进反射后 BRP 能直接读它——排查"点击怎么没反应 / 高亮怎么不消失"时先看它。
#[derive(Resource, Reflect, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[reflect(Resource)]
pub struct PointerOverUi(pub bool);

/// 每帧重算 [`PointerOverUi`]。
///
/// 排在 [`super::InteractionSet`] 链首：同一帧内的拾取、画高亮、写预演读数、
/// 翻译点击看到的是同一个判据（这就是本域注释里说的"看到的画面与点击的解释
/// 用的是同一个悬停格"）。只在状态翻转时写资源，避免每帧触发变化检测。
///
/// **隐藏的区域不算压在 UI 上**：`ui_focus_system` 对不可见节点**不写**
/// `cursor_over`（它只顺手把 `Interaction` 复位就跳过了），所以一个“悬停着就被
/// 收起来”的面板会留下过期的 `cursor_over: true`——那个值绝不能把世界永久冻住
/// （帮助面板 `F1` 开合、敌人行池的 `display: None` 都是这种形状）。
pub fn track_pointer_over_ui_system(
    regions: Query<(&RelativeCursorPosition, Option<&InheritedVisibility>)>,
    mut over: ResMut<PointerOverUi>,
) {
    let next = regions.iter().any(|(cursor, visible)| {
        visible.is_none_or(|visible| visible.get()) && cursor.cursor_over()
    });
    if over.0 != next {
        over.0 = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ui_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<PointerOverUi>()
            .add_systems(Update, track_pointer_over_ui_system);
        app
    }

    /// 光标进出区域时判据跟着翻；**没有声明区域的节点不算数**。
    ///
    /// 真机上 `cursor_over` 由 `bevy_ui` 的 `ui_focus_system` 按几何关系写；
    /// 测试里直接摆出来，因为本系统的职责只是"读它、判一个布尔"。
    #[test]
    fn a_declared_region_under_the_cursor_means_the_pointer_is_over_ui() {
        let mut app = ui_app();
        // 一个没声明 `RelativeCursorPosition` 的节点：它不该影响判据
        let plain = app.world_mut().spawn(Interaction::Hovered).id();
        let region = app
            .world_mut()
            .spawn(RelativeCursorPosition::default())
            .id();

        app.update();
        assert!(
            !app.world().resource::<PointerOverUi>().0,
            "光标不在任何区域上时不该算在 UI 上（哪怕别处有 Interaction）"
        );

        app.world_mut()
            .get_mut::<RelativeCursorPosition>(region)
            .unwrap()
            .cursor_over = true;
        app.update();
        assert!(
            app.world().resource::<PointerOverUi>().0,
            "光标压在一个声明过的区域上 → 指针在 UI 上"
        );

        app.world_mut()
            .get_mut::<RelativeCursorPosition>(region)
            .unwrap()
            .cursor_over = false;
        app.update();
        assert!(
            !app.world().resource::<PointerOverUi>().0,
            "光标离开 → 判据必须复位，否则战场会永远点不动"
        );

        // 反向确认：那个 plain 节点始终没被算进来
        assert!(app.world().get::<Interaction>(plain).is_some());
    }

    /// **卡住的 `Pressed` 不该影响判据**（实机踩到的坑）。
    ///
    /// `Interaction` 在按下时变成 `Pressed`，复位要等松手事件；松手一旦没送到，
    /// 它就一直挂着。判据读 `cursor_over` 因此与点击生命周期无关。
    #[test]
    fn a_stuck_press_does_not_keep_the_world_frozen() {
        let mut app = ui_app();
        app.world_mut().spawn(Interaction::Pressed);

        app.update();

        assert!(
            !app.world().resource::<PointerOverUi>().0,
            "只有 Interaction 卡在 Pressed 时，指针其实不在 UI 上——世界必须还能点"
        );
    }

    /// **收起来的面板不该继续吃掉指针**（实机形状：帮助面板 `F1` 开合）。
    ///
    /// `ui_focus_system` 对不可见节点不写 `cursor_over`，所以“悬停着就被收起来”的
    /// 区域会留下过期的 `true`——不挡住它，世界会永远点不动。
    #[test]
    fn a_hidden_region_stops_holding_the_pointer() {
        let mut app = ui_app();
        let region = app
            .world_mut()
            .spawn((
                RelativeCursorPosition {
                    cursor_over: true,
                    ..default()
                },
                InheritedVisibility::VISIBLE,
            ))
            .id();

        app.update();
        assert!(
            app.world().resource::<PointerOverUi>().0,
            "可见 + 光标在上面 → 压着 UI"
        );

        // 悬停中把它收起来（`cursor_over` 留在 true，因为没人再去写它）
        *app.world_mut()
            .get_mut::<InheritedVisibility>(region)
            .unwrap() = InheritedVisibility::HIDDEN;
        app.update();

        assert!(
            !app.world().resource::<PointerOverUi>().0,
            "收起来之后不算压在 UI 上——否则世界会永久点不动"
        );
    }
}
