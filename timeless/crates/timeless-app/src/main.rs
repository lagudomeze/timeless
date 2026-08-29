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
        .add_message::<menu::ActionSubmitted>()
        .add_message::<combat::HitLanded>()
        .add_message::<combat::RollExecuted>()
        .add_message::<combat::ParryExecuted>()
        .add_message::<timeline::ResetBattle>()
        .add_message::<menu::SelectAction>()
        .add_message::<menu::CommitTurn>()
        .add_message::<menu::ReactionSelect>()
        .add_systems(Startup, (setup::setup, display::hints::configure_gizmos))
        .add_systems(
            Update,
            (
                timeline::reset_system,
                combat::ai_system, // 先于玩家输入决策（威胁判定需要敌人已定指令）
                menu::input_system,
                menu::reaction_system,
                movement::apply_move_intents_system, // 先位移（改变站位）
                combat::resolve_system,              // 后裁决（领域层纯函数）
                movement::projectile_system,         // 投射物每帧推进，命中写入 HitLanded
                combat::death_check_system,          // 清场 + GameOver 判定
                combat::message_log_system,
                display::unit::sync_transforms,
                display::unit::billboard_system,
                display::hints::move_arrow_system,
                display::hud::hud_system,
                display::hud::battle_log_system,
                display::camera::camera_control_system,
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
