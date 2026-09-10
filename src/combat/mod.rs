//! # combat — 战斗领域
//!
//! 按机制分子域，每个子域只装自己的组件 / 消息 / 系统：
//!
//! | 子域 | 回答的问题 |
//! | :--- | :--- |
//! | [`health`] | 谁还有多少血、什么时候死 |
//! | [`formula`] | 一次命中算多少伤害 |
//! | [`attributes`] | 攻击 / 防御的数值属性（伤害、护甲、命中半径） |
//! | [`targeting`] | 打到了谁（碰撞、近战扇形） |
//! | [`lifecycle`] | 攻击实体的存活、命中计数与清理 |
//! | [`skills`] | 生成攻击实体（箭矢、近战横扫） |
//!
//! 流水线（`Update` 内按 [`CombatSet`] 链式执行）：
//!
//! ```text
//! skills（输入 → 生成攻击实体）
//!   ─▶ targeting（挂 CollisionTarget）
//!   ─▶ formula（算伤害 → DamageEvent）
//!   ─▶ lifecycle（命中计数 / 归零速度）
//!   ─▶ health（扣血 → DeathEvent → 销毁实体）
//!   ─▶ lifecycle（清理结束的攻击实体、到期销毁）
//! ```
//!
//! 目标获取只说「打到了谁」，伤害计算只说「打多少」，两者靠临时标记
//! [`CollisionTarget`] 解耦；新增元素伤害只需加一个 formula 系统。

use bevy::prelude::*;

pub mod attributes;
pub mod components;
pub mod formula;
pub mod health;
pub mod lifecycle;
pub mod plugin;
pub mod skills;
pub mod targeting;

pub use attributes::{Armor, HitRadius, PhysicalDamage};
pub use components::{Collidable, Faction};
pub use formula::{DamageEvent, DamageType};
pub use health::{DeathEvent, Health, ModifyHealthEvent};
pub use lifecycle::{HitOnce, Lifetime, Projectile};
pub use plugin::CombatPlugin;
pub use targeting::{CollisionTarget, MeleeShape};

/// 战斗领域在 `Update` 中的系统集。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CombatSet;
