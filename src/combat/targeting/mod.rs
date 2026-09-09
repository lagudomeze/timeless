//! # 目标获取层
//!
//! 只回答「打到了谁」：纯物理检测，不关心敌我、不排除自己 / 其它射弹，
//! 把命中候选以 [`CollisionTarget`](crate::combat::components::CollisionTarget)
//! 临时标记挂到攻击实体上。后续「对波」「友伤过滤」等都是独立扩展点。

pub mod detection;
pub mod melee;

pub use detection::detect_collisions_system;
pub use melee::detect_melee_system;
