//! 攻击生成：把「技能」表达成**技能行动**（载荷 + 工厂 + 执行器）。
//!
//! 玩家 / AI 只**声明**（由 [`crate::timeline`] 调度），到点后执行器生成攻击实体
//! （[`arrow_scene`] / [`melee_scene`] / [`fireball_scene`]）；生成只负责
//! 「摆实体 + 挂组件」，命中与结算完全交给战斗流水线。
//!
//! 两种投射物走两条不同的命中机制：
//!
//! - **箭矢**：碰撞（`CollisionTarget` + `HitRadius`），追踪最近敌人；
//! - **火球**：锁目标格飞行，到达后按**真实距离**结算 AoE（[`explosion`]）。
//!
//! 技能菜单（[`menu`]）与注册表（[`registry`]）也住在这里：技能是战斗领域的概念。

pub mod actions;
pub mod arrow;
pub mod events;
pub mod explosion;
pub mod fireball;
pub mod melee;
pub mod menu;
pub mod registry;

pub use actions::{
    MeleeAction, ShootAction, declare_skill_system, melee_action_executor_system,
    melee_action_scene, shoot_action_executor_system, shoot_action_scene,
};
pub use arrow::arrow_scene;
pub use events::{FireCommand, MeleeCommand};
pub use explosion::{explosion_system, radial_damage_units};
pub use fireball::{
    ARRIVAL_TOLERANCE, FIREBALL_COST, FIREBALL_DAMAGE, FIREBALL_RADIUS, FIREBALL_SPEED, Fireball,
    FireballAction, ProjectileArrived, declare_fireball_at, declare_fireball_system,
    declare_melee_system, fireball_action_executor_system, fireball_action_scene, fireball_scene,
    projectile_arrival_system,
};
pub use melee::{MELEE_DAMAGE, MELEE_FRAME, MELEE_IMPACT, melee_scene};
pub use menu::{
    CycleSkill, MELEE_REACH, MenuSelection, SelectSkill, UseSelectedSkill, cycle_skill_system,
    select_skill_system, skill_line, use_selected_skill_system,
};
pub use registry::{
    PARRY_COST_DISPLAY, SKILLS, SkillDef, SkillKind, affordable_indices, index_of, skill,
};
