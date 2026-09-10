//! 生命值：扣血、治疗与死亡消息。

pub mod components;
pub mod events;
pub mod systems;

pub use components::Health;
pub use events::{DeathEvent, ModifyHealthEvent};
pub use systems::{apply_damage, despawn_dead_system, request_damage_system};
