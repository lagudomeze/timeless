//! 攻击生成：把「技能」表达成**技能行动**（载荷 + 工厂 + 执行器）。
//!
//! 玩家 / AI 只**声明**（调度与到点判定见 [`crate::timeline`]），到点后执行器生成攻击实体
//! （[`arrow_scene`] / [`melee_scene`] / [`fireball_scene`]）；生成只负责
//! 「摆实体 + 挂组件」，命中与结算完全交给战斗流水线。
//!
//! 两种投射物走两条不同的命中机制：
//!
//! - **箭矢**：碰撞（`CollisionTarget` + `HitRadius`），追踪最近敌人；
//! - **火球**：锁目标格飞行，到达后按**真实距离**结算 AoE（[`explosion`]）。
//!
//! 本域装的是**战斗技能的行动**（载荷 + 工厂 + 执行器）与它们的静态定义
//! （[`abilities`]）；目录本身归 [`crate::skills`]，菜单（[`menu`]）留在这里。
//!
//! ⚠️ 待办：把 `registry`（旧的 `SKILLS` 表）并进技能目录，见 `TODO.md` M24。

use bevy::prelude::*;
pub mod abilities;
pub mod actions;
pub mod ammo;
pub mod arrow;
pub mod events;
pub mod explosion;
pub mod fireball;
pub mod melee;
pub mod menu;
pub mod registry;

pub use abilities::{
    ABILITIES as COMBAT_ABILITIES, FIREBALL_ABILITY, MELEE_ABILITY, PARRY_ABILITY, SHOOT_ABILITY,
    register_abilities_system as register_combat_abilities_system,
};
pub use actions::{
    ARROW_TIMING, MELEE_CANCEL_PENALTY, MELEE_TIMING, MeleeAction, ShootAction, declare_melee_at,
    declare_shoot_at, declare_shoot_system, melee_action_executor_system, melee_action_scene,
    refund_melee_observer, shoot_action_executor_system, shoot_action_scene,
};
pub use ammo::{AMMO_MAX, AMMO_RECOVER_INTERVAL, Ammo, AmmoRecoverTimer, recover_ammo_system};
pub use arrow::{ARROW_COST, ARROW_DAMAGE, ARROW_FRAME, ARROW_POWER, ARROW_SPEED, arrow_scene};
pub use events::{FireCommand, MeleeCommand, ShootCommand};
pub use explosion::{explosion_system, radial_damage_units};
pub use fireball::{
    ARRIVAL_TOLERANCE, FIREBALL_AMMO_COST, FIREBALL_DAMAGE, FIREBALL_FRAME, FIREBALL_POWER,
    FIREBALL_RADIUS, FIREBALL_SPEED, FIREBALL_TIMING, Fireball, FireballAction, ProjectileArrived,
    declare_fireball_at, declare_fireball_system, declare_melee_system,
    fireball_action_executor_system, fireball_action_scene, fireball_scene,
    projectile_arrival_system, refund_fireball_observer,
};
pub use melee::{MELEE_DAMAGE, MELEE_FRAME, MELEE_POWER, melee_scene};
pub use menu::{
    CycleSkill, MELEE_REACH, MenuSelection, SelectSkill, UseSelectedSkill, cycle_skill_system,
    select_skill_system, skill_line, use_selected_skill_system,
};
pub use registry::{
    PARRY_COST_DISPLAY, SKILLS, SkillDef, SkillKind, affordable_indices, index_of, skill,
};

pub mod plugin;
pub use plugin::AttackPlugin;

/// 攻击：菜单、声明与执行在本域系统链里的位置（跨子域的先后由 [`CombatPlugin`](super::CombatPlugin) 编排）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AttackSet;
