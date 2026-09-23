//! 攻击实体生命周期：命中计数、到期销毁与清理。

use bevy::prelude::*;
pub mod components;
pub mod systems;

pub use components::{HitOnce, Lifetime, Projectile};
pub use systems::{cleanup_finished_attacks_system, expire_attack_entities_system};

pub mod plugin;
pub use plugin::LifecyclePlugin;

/// 攻击实体清理在本域系统链里的位置（跨子域的先后由 [`CombatPlugin`](super::CombatPlugin) 编排）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LifecycleSet;
