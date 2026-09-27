//! 表现领域插件：资源预载 + 相机平移 + 单位精灵 / 阴影 + 战斗日志 + HUD。

use bevy::prelude::*;

use super::PreloadSet;
use super::PresentationSet;
use super::camera::{
    PanCamera, ZoomCamera, camera_follow_system, camera_pan_system, camera_zoom_system,
};
use super::components::MainCamera;
use super::effects::{
    animate_damage_numbers_system, animate_hit_effects_system, spawn_hit_effects_system,
};
use super::hud;
use super::hud::hint::{
    HintState, HintTimer, PreviewReadout, apply_hint_system, gather_hint_system,
};
use super::hud::timeline::{
    TimelineFocusRing, TimelineHover, TimelineLayout, TimelineReadoutState,
    update_timeline_focus_ring_system,
};
use super::hud::{
    HudCache, ToggleHelp, fit_ui_scale_system, setup_hud, toggle_help_system, toggle_log_system,
    update_action_labels_system, update_log_panel_system, update_skill_bar_system,
    update_timeline_readout_system, update_timeline_system, update_unit_panels_system,
};
use super::log::{BattleLog, battle_log_system};
use super::preload::preload;
use super::threat_grid::{
    spawn_threat_source_rings, spawn_threat_tiles, update_threat_grid_system,
    update_threat_source_ring_system,
};
use super::unit_sprite::{
    FactionRing, UnitShadow, UnitSprite, UnitSprites, billboard_system, shadow_system,
};
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
            // AI 状态也进反射：排查"敌人为什么不动"时能直接读战术
            .register_type::<crate::ai::EnemyBrain>()
            .register_type::<crate::ai::Tactic>()
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
            .register_type::<hud::timeline::TimelineReadout>()
            .register_type::<hud::timeline::TimelineReadoutText>()
            .register_type::<TimelineFocusRing>()
            .register_type::<crate::presentation::ThreatTile>()
            .register_type::<crate::presentation::ThreatSourceRing>()
            .register_type::<hud::log_panel::LogPanel>()
            .register_type::<hud::log_panel::LogCollapsed>()
            .register_type::<hud::log_panel::LogHeaderButton>()
            .register_type::<hud::log_panel::LogHeaderLabel>()
            .register_type::<hud::log_panel::LogBody>()
            .register_type::<hud::log_panel::LogBodyText>()
            .register_type::<hud::hint::ActionHint>()
            .register_type::<hud::hint::ActionHintText>()
            .register_type::<hud::help::HelpPanel>()
            // 诊断锚点（见 docs/backlog/clock.md 的 #62）：日志内容、素材句柄、
            // 主相机实体——远程核查时按相机截图 / 读日志原文都要靠它们
            .register_type::<BattleLog>()
            .register_type::<UnitSprites>()
            // 单位外观的三个零件：BRP 靠它们找"这个单位的纸片 / 阴影 / 阵营环在哪"
            // （没派生 Reflect 的组件在远程协议里等于不存在——`world.query` 会静默返回空）
            .register_type::<UnitSprite>()
            .register_type::<UnitShadow>()
            .register_type::<FactionRing>()
            .register_type::<MainCamera>()
            // 「无法操作」提示的计时器：走真实时间，冻结时也要能自己消失
            .init_resource::<HintTimer>()
            // 提示条的取数结果（一帧一份：被拒输入 / 威胁窗口 / 预演读数取一个）
            .init_resource::<HintState>()
            // 时间轴悬停：布局（色块 → 行动实体）与读数快照都是本域内部资源
            .init_resource::<TimelineLayout>()
            .init_resource::<TimelineReadoutState>()
            .init_resource::<TimelineHover>()
            // 预演读数：写方是 interaction，消费方是本域
            .add_message::<PreviewReadout>()
            // 方块交互被拒：写方是 world（纯数据域，不认识时间线的 ActionBlocked），
            // 消费方是本域——表现层替它把原因翻译成同一条提示条上的文案
            .add_message::<crate::world::BlockRefused>()
            // 走不过去（地形太高）：写方是 movement，消费方是本域的提示条
            .add_message::<crate::movement::MoveRefused>()
            .add_message::<PanCamera>()
            .add_message::<ZoomCamera>()
            // 帮助面板的开关消息：写方是 input（F1），消费方是 HUD
            .add_message::<ToggleHelp>()
            .add_systems(
                Startup,
                // HUD 用单位精灵当头像，必须跑在资源预载之后
                (
                    preload.in_set(PreloadSet),
                    setup_hud.after(preload),
                    spawn_threat_tiles,
                    // 威胁**来源**圈：与威胁格同一个功能的两半（"谁在打我" / "它打哪"）
                    spawn_threat_source_rings,
                ),
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
                    // 命中特效：读 `DamageEvent`，在被打中的单位身上冒一簇短命粒子
                    // 与一个伤害数字（字体由 `setup_hud` 放进 `EffectFont`）。
                    // 三个系统收成一条链：先建、再动，也避免 Update 的元组超过 Bevy 上限。
                    (
                        spawn_hit_effects_system,
                        animate_hit_effects_system,
                        animate_damage_numbers_system,
                    )
                        .chain(),
                    // 威胁格：把敌人这一手威胁到的格画在地面上（只读 `Threatens`）
                    update_threat_grid_system,
                    // 威胁来源圈：同一个窗口开着时，圈出**是谁**在威胁你
                    update_threat_source_ring_system,
                    update_unit_panels_system,
                    update_action_labels_system,
                    update_skill_bar_system,
                    update_timeline_system,
                    // 悬停读数跑在时间轴之后：它读的就是刚算出来的那份布局
                    update_timeline_readout_system,
                    // 指示圈读 `TimelineHover`（上面那个系统刚写的），所以排在它之后
                    update_timeline_focus_ring_system,
                    toggle_log_system,
                    update_log_panel_system,
                    // 提示条两层：取数（被拒输入 > 威胁窗口 > 预演读数）→ 写节点。
                    // 取数必须排在各声明系统之后，才拿得到本帧的拒绝原因。
                    (gather_hint_system, apply_hint_system).chain(),
                    toggle_help_system,
                    fit_ui_scale_system,
                )
                    .chain()
                    .in_set(PresentationSet),
            );
    }
}
