//! 生命值：扣血与死亡消息。

pub mod components;
pub mod events;
pub mod systems;

pub use components::Health;
pub use events::DeathEvent;
pub use systems::{apply_damage_system, despawn_dead_system};
