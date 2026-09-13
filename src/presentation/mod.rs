//! # presentation — 表现领域（相机 / 单位精灵 / 3D 装饰 / UI / 日志）
//!
//! 「怎么给人看」：相机与灯光、单位的 2D 纸片与贴地阴影、地表装饰、战斗日志
//! （将来还有血条、动画、特效）。本域**只读**游戏状态、只写表现，不裁决规则、
//! 不改游戏数据。
//!
//! 体素地图的表现不在这里（那是 [`crate::voxel_render`] 的职责）。
//! 实体的**组装**也不在这里（那是 [`crate::spawn`] 的职责）——本域只提供零件：
//! 相机场景、装饰模型表、日志资源。

use bevy::prelude::*;

pub mod camera;
pub mod components;
pub mod decoration;
pub mod hud;
pub mod log;
pub mod plugin;
pub mod preload;
pub mod unit_sprite;

pub use camera::{PanCamera, ZoomCamera};
pub use components::{CameraRig, MainCamera};
pub use hud::{HudRoot, ToggleHelp, setup_hud, toggle_help_system};
pub use log::{BattleLog, battle_log_system};
pub use plugin::PresentationPlugin;
pub use unit_sprite::{UnitShadow, UnitSprite, UnitSprites};

/// 表现域在 `Update` 中的系统集（整条游戏流水线之后：读结果、不改结果）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PresentationSet;

/// 资源预载（Startup）：必须早于 [`SpawnSet`](crate::spawn::AssemblySet) 的场景组装。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreloadSet;
