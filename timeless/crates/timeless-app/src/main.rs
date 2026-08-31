//! # Project Timeless — 应用层入口
//!
//! 进度：Phase 1.5 伪 3D 场景（3D 场景 + 2D 纸片单位）已替换原 5×5 方块原型。
//! 按领域组织，每个文件包含该领域的组件/消息/系统：
//! `combat`（战斗）、`movement`（移动/投射物）、`timeline`（时间线）、
//! `menu`（暂停菜单）、`display`（展示层，按子域拆分为 mod 目录），
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

use menu::MenuSelection;
use timeline::TimeLineState;

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
        .init_resource::<TimeLineState>()
        .init_resource::<MenuSelection>()
        .init_resource::<combat::BattleLog>()
        .add_message::<combat::HitLanded>()
        .add_message::<combat::ProjectileArrived>()
        .add_message::<timeline::ResetBattle>()
        .add_message::<timeline::TurnCommitted>()
        .add_message::<menu::SelectSkill>()
        .add_message::<menu::CycleSkill>()
        .add_message::<movement::MoveInput>()
        .add_message::<menu::CommitTurn>()
        .add_message::<menu::ReactionSelect>()
        .add_systems(
            Startup,
            (
                setup::setup,
                display::hints::configure_gizmos,
                timeline::start_paused, // 开局 Decision 冻结虚拟时间
            ),
        )
        .add_systems(
            Update,
            (
                (
                    timeline::sync_pause_system, // 按阶段同步 Time<Virtual> 暂停
                    timeline::reset_system,
                    combat::ai_system,              // Decision：敌人声明动作实体
                    menu::decision_keyboard_system, // 键盘 → 消息（CycleSkill / MoveInput / CommitTurn）
                    menu::select_skill_system,      // Tab / 面板 → 声明动作实体
                    movement::move_input_system,    // MoveInput → Declared MoveTo
                    menu::commit_system,            // 提交校验 + 扣费 → TurnCommitted
                    timeline::phase_advance_system, // TurnCommitted → Resolving（时间放行）
                    timeline::finalize_declared_actions, // Declared → Pending（分配 execute_at）
                    combat::reaction_trigger_system, // 双方 Pending 攻击 → Reaction（暂停）
                    menu::reaction_keyboard_system, // 反应阶段键盘 → ReactionSelect
                    menu::reaction_execution_system, // 反应执行：继续 / 翻滚取消 / 招架
                ),
                (
                    timeline::scheduler,                // Pending → Committed（时间到期）
                    movement::move_executor,            // MoveTo：新位置 = 当前位置 + 速度
                    movement::roll_executor,            // Roll：后退 + Dodging 闪避标记
                    combat::parry_executor,             // Parry：挂 Parrying 招架标记
                    combat::combat_phase1_system,       // 阶段 1 计算（只读 + CombatResult）
                    combat::fireball_executor,          // Fireball → 生成投射物
                    combat::combat_phase2_system,       // 阶段 2 应用（扣血 + despawn）
                    movement::projectile_motion_system, // 位置+速度自动飞行 → ProjectileArrived
                    combat::explosion_system,           // 到达爆炸：范围伤害 + 销毁投射物
                    combat::death_check_system,         // 清场 + GameOver
                    timeline::turn_end_system,          // 动作清空 → 下一回合 Decision
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
