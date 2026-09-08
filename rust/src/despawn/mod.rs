//! # 实体销毁模块
//!
//! 监听 [`DeathEvent`](crate::events::DeathEvent)，把死亡实体从世界移除。

pub mod systems;

pub use systems::despawn_dead_system;
