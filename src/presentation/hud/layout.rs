//! HUD 布局：整套 Node 层级在这里拼（相当于 Unity 的 Canvas Hierarchy）。
//!
//! ```text
//! HudRoot                      整屏、不吃鼠标事件
//! ├── Timeline               (顶部)    状态行 + 时间轴轨道（色块池）
//! ├── PlayerPanel            (左下)    头像 + 状态行 + HP / EN 条 + 行动行
//! ├── Enemy1Row .. EnemyNRow (右下)    **每个敌人一行**（没有头像：N 行会太高，
//! │                                   且行首的 `ENEMY 1` 已经能分清是谁）
//! ├── SkillBar               (底部中)  4 个图标按钮 + 角标 + 悬停 tooltip
//! ├── LogPanel               (右下偏上) 标题按钮 + 正文（可折叠）
//! └── HelpPanel              (居中)    F1 开合
//! ```
//!
//! 分辨率适配：所有面板都用「像素尺寸 + 百分比锚点」，再由
//! [`fit_ui_scale_system`] 按窗口高度缩放整套 UI（等价于 UGUI 的
//! Canvas Scaler = Scale With Screen Size，Reference Resolution = [`BASE_HEIGHT`]）。

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy::window::PrimaryWindow;

use crate::combat::Faction;
use crate::presentation::unit_sprite::UnitSprites;

use super::{
    BASE_HEIGHT, HUD_FONT, MAX_UI_SCALE, MIN_UI_SCALE, help, hint, log_panel, panels, skills,
    timeline,
};

/// HUD 根标记（整屏容器）。
#[derive(Component, Default, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct HudRoot;

/// Startup 一次：组装整套 HUD（在资源预载之后跑，因为要用单位精灵当头像）。
///
/// 根节点用 BSN 建（`bsn!` + `spawn_scene`）；六个区域仍是 `impl Bundle` 的工厂，
/// 用 `add_children` 挂上去——`Children [...]` 要求每个子节点是 `Scene` 而不是
/// `Bundle`，把六个工厂全改成场景是另一件独立的事（见 `docs/` 的 BSN 待办）。
pub fn setup_hud(mut commands: Commands, assets: Res<AssetServer>, sprites: Res<UnitSprites>) {
    let font: Handle<Font> = assets.load(HUD_FONT);
    let root = commands
        .spawn_scene(bsn! {
            Name("HudRoot")
            template_value(HudRoot)
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
            }
            // 根只是布局容器：默认的 Block 会把整屏的鼠标事件吃掉
            template_value(FocusPolicy::Pass)
        })
        .id();

    let regions = [
        commands
            .spawn(panels::unit_panel(&font, sprites.sprite(Faction::Player)))
            .id(),
        commands.spawn(skills::skill_bar(&font, &assets)).id(),
        commands.spawn(log_panel::log_panel(&font)).id(),
        commands.spawn(hint::hint_panel(&font)).id(),
        commands.spawn(help::help_panel(&font)).id(),
    ];
    commands.entity(root).add_children(&regions);

    // 敌人面板是**行池**（按下标建好、每帧只改内容与显隐），所以不走上面的工厂数组——
    // 池子大小由 `MAX_ENEMY_ROWS` 一处说了算（有测试钉住两边一致）
    let enemy_column = commands.spawn(panels::enemy_column()).id();
    let enemy_rows: Vec<Entity> = (0..panels::MAX_ENEMY_ROWS)
        .map(|index| commands.spawn(panels::enemy_row(&font, index)).id())
        .collect();
    commands.entity(enemy_column).add_children(&enemy_rows);
    commands.entity(root).add_child(enemy_column);

    // 时间轴要建一个色块池（循环 spawn），所以不走上面的工厂数组
    let timeline = timeline::spawn_timeline(&mut commands, &font);
    commands.entity(root).add_child(timeline);

    // 悬停时间轴时圈出战场上那个单位（世界空间的一次性指示，默认隐藏）
    timeline::spawn_timeline_focus_ring(&mut commands);
}

/// 窗口高度 → `UiScale`：高分屏上整套 HUD 等比放大，窗口再大也不会散架。
pub fn fit_ui_scale_system(
    scale: Option<ResMut<UiScale>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let Some(mut scale) = scale else {
        return; // 无 UI 插件的环境（部分单测）直接跳过
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let target = (window.height() / BASE_HEIGHT).clamp(MIN_UI_SCALE, MAX_UI_SCALE);
    if (scale.0 - target).abs() > f32::EPSILON {
        scale.0 = target;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::window::WindowResolution;

    #[test]
    fn ui_scale_tracks_the_window_height() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(UiScale::default())
            .add_systems(Update, fit_ui_scale_system);
        app.world_mut().spawn((
            Window {
                resolution: WindowResolution::new(1280, 1440),
                ..default()
            },
            PrimaryWindow,
        ));

        app.update();
        assert_eq!(
            app.world().resource::<UiScale>().0,
            2.0f32.min(MAX_UI_SCALE),
            "1440 高 = 设计高度的 2 倍，应当放大（并夹到上限）"
        );

        app.world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
            .unwrap()
            .resolution = WindowResolution::new(1280, 360);
        app.update();
        assert_eq!(
            app.world().resource::<UiScale>().0,
            0.5f32.max(MIN_UI_SCALE),
            "小窗口应当缩小（并夹到下限）"
        );
    }

    /// 整机：五个区域的节点都挂上了预期的 `Name`。
    ///
    /// 这些名字是运行时诊断的定位锚点（BRP 的 `world.query` / 按名截图）——改名或漏挂
    /// 不会让屏幕变化，却会让「这条 UI 到底是谁」重新变成猜谜，所以在这里钉死。
    #[test]
    fn setup_hud_names_every_region_it_builds() {
        let mut app = crate::test_support::headless_app();
        app.update(); // Startup：preload + setup_hud

        let mut query = app.world_mut().query::<&Name>();
        let names: Vec<String> = query
            .iter(app.world())
            .map(|name| name.as_str().to_string())
            .collect();

        for expected in [
            "HudRoot",
            // 左下 / 右下：面板、头像、状态行、两条进度条（轨道 + 填充 + 文本）、行动行
            "PlayerPanel",
            "PlayerPortrait",
            "PlayerInfo",
            "PlayerStateLine",
            "PlayerHpBar",
            "PlayerHpFill",
            "PlayerHpText",
            "PlayerEnFill",
            "PlayerAction",
            // 右下：敌人**行池**（每行一个 `UnitPanel` 手法，见 `MAX_ENEMY_ROWS`）
            "Enemy1Row",
            "Enemy1Info",
            "EnemyPanels",
            "Enemy1StateLine",
            "Enemy1InsightLine",
            "Enemy1HpFill",
            "Enemy1EnFill",
            "Enemy1Action",
            // 正下方：技能栏容器 + 4 个槽位（图标 / 热键 / 角标）+ tooltip
            "SkillBar",
            "SkillBarPanel",
            "SkillSlot0",
            "SkillSlot0Icon",
            "SkillSlot0Hotkey",
            "SkillSlot0Badge",
            "SkillSlot3Badge",
            "SkillTooltip",
            "SkillTooltipText",
            // 右下偏上：可折叠日志
            "CombatLog",
            "CombatLogHeader",
            "CombatLogHeaderLabel",
            "CombatLogBody",
            "CombatLogBodyText",
            // 左下角上方：被拒输入的提示条
            "ActionHint",
            "ActionHintText",
            // 居中：F1 帮助
            "HelpPanel",
            "HelpTitle",
            "HelpKeys",
            // 顶部：时间轴 + 色块池两端
            "Timeline",
            "TimelineState",
            "TimelineRows",
            "TimelineLaneLabels",
            "TimelineLanes",
            "TimelineStaging",
            "TimelineLane0",
            "TimelineLane3",
            "TimelineLaneLabel0",
            "TimelineTick1",
            "TimelinePlayhead",
            "TimelineBlockLane0Slot0",
            "TimelineBlockLabelLane3Slot0",
            "TimelineBlockMarkLane0Slot1",
            "TimelineReady0",
            "TimelineReadyLabel0",
        ] {
            assert!(
                names.iter().any(|name| name == expected),
                "HUD 缺少名为 {expected} 的节点；实际有 {} 个具名节点",
                names.len()
            );
        }
    }
}
