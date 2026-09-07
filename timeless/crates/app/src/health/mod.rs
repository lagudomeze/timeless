//! # 血量模块（核心底层）
//!
//! 只做一件事：持有 [`Health`] 组件，消费 [`DamageEvent`] 扣血，
//! 生命归零时发出 [`DeathEvent`]。不关心伤害从哪来、由哪种机制造成。

pub mod components;
pub mod systems;

pub use components::Health;
pub use systems::apply_damage_system;
