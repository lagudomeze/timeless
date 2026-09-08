//! # 战斗模块：世界空间流水线（蓝图落地，Bevy 0.19）
//!
//! 数据流（`Update` 内 `.chain()` 顺序执行）：
//!
//! ```text
//! move_entities ─▶ detect_collisions（挂 CollisionTarget）
//!   ─▶ damage::physical（计算护甲 → DamageEvent）
//!   ─▶ lifecycle::manage_projectile_hits（计数/归零/移除标记）
//!   ─▶ health::apply_damage（扣血 → DeathEvent）
//!   ─▶ cleanup（销毁 finished 投射物）
//!   ─▶ despawn::despawn_dead（销毁死亡实体）
//! ```
//!
//! 目标获取（targeting）只回答「打到了谁」，伤害计算（damage）只回答
//! 「打多少血」，两者通过临时标记 [`components::CollisionTarget`] 解耦。

pub mod cleanup;
pub mod components;
pub mod damage;
pub mod faction;
pub mod lifecycle;
pub mod movement;
pub mod targeting;

pub use cleanup::cleanup_finished_attacks_system;
pub use components::{
    Armor, Collidable, CollisionTarget, HitOnce, HitRadius, Lifetime, MeleeShape, Owner,
    PhysicalDamage, Projectile, Velocity,
};
pub use damage::apply_physical_damage_system;
pub use faction::Faction;
pub use lifecycle::{expire_attack_entities_system, manage_projectile_hits_system};
pub use movement::move_entities_system;
pub use targeting::{detect_collisions_system, detect_melee_system};
