//! 目标获取：只回答「打到了谁」，不关心敌我与伤害数值。
//!
//! 结果统一以 [`CollisionTarget`] 临时标记挂在攻击实体上：
//! 伤害计算只认这个标记，因此「对波」「友伤」「AOE」都是独立扩展点。

pub mod components;
pub mod detection;
pub mod melee;

pub use components::{CollisionTarget, MeleeShape};
pub use detection::detect_collisions_system;
pub use melee::detect_melee_system;
