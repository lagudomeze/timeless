//! # defense — 防御机制（翻滚 / 招架）
//!
//! 「见招拆招」的落地：两个**反应性动作** + 两个瞬时标记。
//!
//! | 动作 | 消耗 | 效果 | 标记 |
//! | :--- | ---: | :--- | :--- |
//! | 翻滚 | 1 精力 | 远离威胁退一格 | [`Dodging`]（无敌帧 `DODGE_SECS`） |
//! | 招架 | 1 精力 | 挡下绑定的那次攻击并反制 | [`Parrying`]（绑实体 + 兜底过期） |
//!
//! 判定链在 [`crate::combat::formula::resolution`]（两阶段结算）：
//! `CollisionTarget` → 阶段 1 只读裁决 → 阶段 2 统一落地 → `apply_damage`（唯一扣血入口）。
//!
//! 本域**不认识护甲**（那是 [`crate::combat::formula`] 的事），
//! 伤害公式也**不认识防御**——两边在领域层用 `DefenseState` 交接。

use bevy::prelude::*;

pub mod actions;
pub mod components;
pub mod events;
pub mod stamina;
pub mod systems;

pub use actions::{
    DODGE_SECS, PARRY_COST, PARRY_SECS, ROLL_COST, declare_parry_system, declare_roll_system,
    parry_executor_system, roll_executor_system,
};
pub use components::{AttackResolved, DefenseOutcome, Dodging, ParryAction, Parrying};
pub use events::{ParryCommand, RollCommand};
pub use stamina::{STAMINA_REGEN_PER_DECISION, Stamina};
pub use systems::expire_defense_markers_system;

/// 招架行动工厂：载荷 + 调度数据 + 草案标记。
pub fn parry_action_scene(actor: Entity, target_attack: Entity, now: f32) -> impl Scene {
    let schedule =
        crate::timeline::ScheduledAction::declared_at(actor, crate::timeline::timing::PARRY, now);

    bsn! {
        ParryAction {
            target_attack: {target_attack},
        }
        template_value(schedule)
        crate::timeline::Declared
    }
}
