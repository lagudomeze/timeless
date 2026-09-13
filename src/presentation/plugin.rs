//! 表现领域插件：资源预载 + 相机平移 + 单位精灵 / 阴影 + 战斗日志 + HUD。

use bevy::prelude::*;

use super::PreloadSet;
use super::PresentationSet;
use super::camera::{
    PanCamera, ZoomCamera, camera_follow_system, camera_pan_system, camera_zoom_system,
};
use super::hud;
use super::hud::hint::{HintTimer, PreviewReadout, update_action_hint_system};
use super::hud::{
    HudCache, ToggleHelp, fit_ui_scale_system, setup_hud, toggle_help_system, toggle_log_system,
    update_action_labels_system, update_log_panel_system, update_skill_bar_system,
    update_timeline_system, update_unit_panels_system,
};
use super::log::{BattleLog, battle_log_system};
use super::preload::preload;
use super::unit_sprite::{billboard_system, shadow_system};
use crate::combat::Faction;

/// 表现领域插件。
#[derive(Debug, Default)]
pub struct PresentationPlugin;

impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BattleLog>()
            // HUD 写入缓存：只有内容变了才碰 UI 节点
            .init_resource::<HudCache>()
            // BRP 诊断锚点：**没注册的组件在远程协议里等于不存在**（`world.query`
            // 既不能拿它当过滤器，也取不到数据），注册后才能在运行时按组件
            // 直接定位「玩家 HP 条」「技能槽 3」这类实体，配合各节点的 `Name`
            // 就不必再靠实体 ID 反推 UI 树。
            .register_type::<Faction>()
            // AI 状态也进反射：排查"敌人为什么不动"时能直接读意图
            .register_type::<crate::ai::EnemyBrain>()
            .register_type::<crate::ai::Intent>()
            .register_type::<hud::layout::HudRoot>()
            .register_type::<hud::panels::UnitPanel>()
            .register_type::<hud::panels::PanelBar>()
            .register_type::<hud::panels::PanelText>()
            .register_type::<hud::actions::ActionLabel>()
            .register_type::<hud::skills::SkillSlot>()
            .register_type::<hud::skills::SkillBadge>()
            .register_type::<hud::skills::SkillTooltip>()
            .register_type::<hud::skills::SkillTooltipText>()
            .register_type::<hud::timeline::TimelineStateLabel>()
            .register_type::<hud::timeline::TimelineLane>()
            .register_type::<hud::timeline::TimelineLaneLabel>()
            .register_type::<hud::timeline::TimelineBlock>()
            .register_type::<hud::timeline::TimelineBlockLabel>()
            .register_type::<hud::timeline::TimelineBlockMark>()
            .register_type::<hud::timeline::TimelineReadyChip>()
            .register_type::<hud::timeline::TimelineReadyLabel>()
            .register_type::<hud::log_panel::LogPanel>()
            .register_type::<hud::log_panel::LogCollapsed>()
            .register_type::<hud::log_panel::LogHeaderButton>()
            .register_type::<hud::log_panel::LogHeaderLabel>()
            .register_type::<hud::log_panel::LogBody>()
            .register_type::<hud::log_panel::LogBodyText>()
            .register_type::<hud::hint::ActionHint>()
            .register_type::<hud::hint::ActionHintText>()
            .register_type::<hud::help::HelpPanel>()
            // 「无法操作」提示的计时器：走真实时间，冻结时也要能自己消失
            .init_resource::<HintTimer>()
            // 预演读数：写方是 interaction，消费方是本域
            .add_message::<PreviewReadout>()
            .add_message::<PanCamera>()
            .add_message::<ZoomCamera>()
            // 帮助面板的开关消息：写方是 input（F1），消费方是 HUD
            .add_message::<ToggleHelp>()
            .add_systems(
                Startup,
                // HUD 用单位精灵当头像，必须跑在资源预载之后
                (preload.in_set(PreloadSet), setup_hud.after(preload)),
            )
            .add_systems(
                Update,
                (
                    camera_pan_system,
                    camera_zoom_system,
                    camera_follow_system,
                    billboard_system,
                    shadow_system,
                    battle_log_system,
                    update_unit_panels_system,
                    update_action_labels_system,
                    update_skill_bar_system,
                    update_timeline_system,
                    toggle_log_system,
                    update_log_panel_system,
                    update_action_hint_system,
                    toggle_help_system,
                    fit_ui_scale_system,
                )
                    .chain()
                    .in_set(PresentationSet),
            );
    }
}
