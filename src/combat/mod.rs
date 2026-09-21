//! # combat — 战斗领域
//!
//! 按机制分子域，每个子域只装自己的组件 / 消息 / 系统：
//!
//! | 子域 | 回答的问题 |
//! | :--- | :--- |
//! | [`health`] | 谁还有多少血、什么时候死 |
//! | [`formula`] | 一次命中算多少伤害（纯公式 + 每种伤害类型的命中系统） |
//! | [`attributes`] | 攻击 / 防御的数值属性（伤害、护甲、命中半径、打断力度） |
//! | [`targeting`] | 打到了谁（碰撞、近战扇形） |
//! | [`lifecycle`] | 攻击实体的存活、命中计数与清理 |
//! | [`skills`] | 生成攻击实体（箭矢、横扫、火球） |
//! | [`defense`] | 翻滚 / 招架与它们的短命标记 |
//! | [`reaction`] | 威胁检测：有东西瞄准玩家 → 请求冻结世界 |
//!
//! 流水线（`Update` 内按 [`CombatSet`] 链式执行）：
//!
//! ```text
//! reaction（检测威胁 → 请求冻结）
//!   ─▶ skills（声明 → 到点生成攻击实体 / 火球投射物）
//!   ─▶ projectile_arrival + explosion（到达目标格 → 按真实距离 AoE）
//!   ─▶ targeting（挂 CollisionTarget）
//!   ─▶ apply_physical_hits（防御判定 + 护甲 + 打断触发 + 命中计数）
//!   ─▶ health（扣血 → DeathEvent）→ despawn_dead（帧末销毁）
//!   ─▶ lifecycle（清理结束的攻击实体、到期销毁）
//! ```
//!
//! 目标获取只说「打到了谁」，伤害只说「打多少」，两者靠临时标记
//! [`CollisionTarget`] 解耦。**没有两阶段裁决**：伤害是纯减法（可交换），
//! 不需要快照，也不需要"阶段 1 只读 / 阶段 2 落地"——被打断这件事改由
//! [`InterruptEvent`](crate::combat::formula::InterruptEvent) 打**还没到点的行动**。

use bevy::prelude::*;

pub mod attributes;
pub mod components;
pub mod defense;
pub mod formula;
pub mod health;
pub mod lifecycle;
pub mod plugin;
pub mod reaction;
pub mod skills;
pub mod targeting;

pub use attributes::{Armor, AttackFrame, AttackRange, HitRadius, InterruptPower, PhysicalDamage};
pub use components::{Collidable, Faction};
pub use defense::{DefenseOutcome, Dodging, ParryCommand, Parrying, RollCommand, Stamina};
pub use formula::{
    DefenseState, InterruptEvent, counter_damage, interrupt_lands, physical_damage, resolve_defense,
};
pub use health::{DamageEvent, DeathEvent, Health, apply_damage_system, despawn_dead_system};
pub use lifecycle::{HitOnce, Lifetime, Projectile};
pub use plugin::CombatPlugin;
pub use reaction::{
    TargetCell, ThreatWindow, Threatened, Threatens, detect_threat_system, mark_threatened_system,
    melee_arc_cells, trajectory_cells,
};
pub use skills::{
    FIREBALL_COST, FireCommand, Fireball, FireballAction, MeleeAction, MeleeCommand, MenuSelection,
    ProjectileArrived, SKILLS, ShootAction, SkillDef, SkillKind,
};
pub use targeting::{CollisionTarget, MeleeShape};

/// 战斗领域在 `Update` 中的系统集。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CombatSet;
