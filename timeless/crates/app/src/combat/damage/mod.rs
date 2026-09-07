//! # 伤害计算层
//!
//! 每种伤害类型一个模块 + 一个系统：只读取「自己的伤害组件 + 目标防御组件」，
//! 算出最终数值后发 [`DamageEvent`](crate::events::DamageEvent)。
//! 不关心目标来自碰撞还是 AOE（认 `CollisionTarget` 标记即可）。

pub mod physical;

pub use physical::apply_physical_damage_system;
