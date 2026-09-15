//! 攻击实体生命周期：命中计数、到期销毁与清理。

pub mod components;
pub mod systems;

pub use components::{HitOnce, Lifetime, Projectile};
pub use systems::{cleanup_finished_attacks_system, expire_attack_entities_system};
