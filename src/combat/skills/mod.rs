//! 攻击生成：把「技能」表达成**技能行动**（载荷 + 工厂 + 执行器）。
//!
//! 规划阶段玩家只**声明**（由 [`crate::timeline`] 调度），到点后执行器生成攻击实体
//! （[`arrow_scene`] / [`melee_scene`]）；生成只负责「摆实体 + 挂组件」，
//! 命中与结算完全交给战斗流水线。

pub mod actions;
pub mod arrow;
pub mod events;
pub mod melee;

pub use actions::{
    MeleeAction, ShootAction, declare_skill_system, melee_action_executor_system,
    melee_action_scene, shoot_action_executor_system, shoot_action_scene,
};
pub use arrow::arrow_scene;
pub use events::{FireCommand, MeleeCommand};
pub use melee::melee_scene;
