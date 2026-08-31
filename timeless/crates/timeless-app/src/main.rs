//! # Project Timeless — 应用层入口
//!
//! 无回合设计：`Time<Virtual>` 持续流动，行动实体（载荷 + `ScheduledAction` +
//! `Declared → Pending → Committed`）按虚拟时间调度执行，不存在回合 / 阶段状态机。
//! 按领域组织，每个文件包含该领域的组件/消息/系统：
//! `combat`（战斗）、`movement`（移动/投射物）、`timeline`（时间线调度）、
//! `menu`（技能菜单与实时反应）、`display`（展示层，按子域拆分为 mod 目录），
//! 场景搭建见 `setup`，领域层裁决在 `timeless-domain`。

mod combat;
mod debug;
mod display;
mod menu;
mod movement;
mod setup;
mod timeline;

use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};

fn main() {
    App::new()
        // 资产根目录固定指向本 crate 的 assets/（编译期绝对路径）：
        // 无论 `cargo run -p timeless-app` 还是直接运行 exe 都能找到素材。
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").to_string(),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .insert_resource(ClearColor(Color::srgb(0.12, 0.14, 0.18)))
        .init_resource::<menu::MenuSelection>()
        .init_resource::<combat::BattleLog>()
        .add_message::<combat::HitLanded>()
        .add_message::<combat::ProjectileArrived>()
        .add_message::<timeline::ResetBattle>()
        .add_message::<timeline::ActionsCommitted>()
        .add_message::<menu::SelectSkill>()
        .add_message::<menu::CycleSkill>()
        .add_message::<movement::MoveInput>()
        .add_message::<menu::CommitAction>()
        .add_message::<menu::ReactionInput>()
        .add_systems(Startup, (setup::setup, display::hints::configure_gizmos))
        .add_systems(
            Update,
            (
                (
                    timeline::reset_system,
                    combat::ai_system, // 敌人动作清空后直接入队 Pending
                    menu::decision_keyboard_system, // 键盘 → CycleSkill / MoveInput / CommitAction
                    menu::reaction_input_system, // Q/E → ReactionInput
                    menu::select_skill_system, // Tab / 面板 → Declared 草案
                    movement::move_input_system, // MoveInput → Declared MoveTo
                    menu::commit_system, // 提交校验 + 扣费 → ActionsCommitted
                    timeline::finalize_declared_actions, // ActionsCommitted → Pending（分配 execute_at）
                    menu::reaction_execution_system,     // 实时翻滚取消 / 招架（前摇窗口）
                ),
                (
                    timeline::scheduler,                   // Pending → Committed（虚拟时间到期）
                    movement::move_executor,               // MoveTo：新位置 = 当前位置 + 速度
                    movement::roll_executor,               // Roll：后退 + Dodging 闪避标记
                    combat::parry_executor,                // Parry：挂 Parrying 招架标记
                    combat::combat_phase1_system,          // 阶段 1 计算（只读 + CombatResult）
                    combat::fireball_executor,             // Fireball → 生成投射物
                    combat::combat_phase2_system,          // 阶段 2 应用（扣血 + despawn）
                    movement::projectile_motion_system,    // 位置+速度自动飞行 → ProjectileArrived
                    combat::explosion_system,              // 到达爆炸：范围伤害 + 销毁投射物
                    combat::death_check_system,            // 清场 + 战斗结束暂停虚拟时间
                    combat::expire_defense_markers_system, // Dodging / Parrying 过期清理
                    combat::message_log_system,
                    display::unit::sync_transforms,
                    display::unit::billboard_system,
                    display::hints::move_arrow_system,
                    display::hud::hud_system,
                    display::hud::battle_log_system,
                    display::camera::camera_control_system,
                ),
            )
                .chain(),
        )
        .add_systems(EguiPrimaryContextPass, debug::debug_panel_system)
        // 悬停坐标依赖相机全局变换：等 Transform 传播后再投影（避免拖拽时滞后一帧）
        .add_systems(
            PostUpdate,
            display::hover::hover_info_system.after(TransformSystems::Propagate),
        )
        .run();
}
