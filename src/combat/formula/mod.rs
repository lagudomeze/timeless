//! 伤害计算：数值公式与伤害消息。

pub mod events;
pub mod systems;
pub mod types;

pub use events::DamageEvent;
pub use systems::{apply_physical_damage_system, physical_damage};
pub use types::DamageType;
