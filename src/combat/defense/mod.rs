//! # defense — 防御机制（翻滚 / 招架）
//!
//! 「见招拆招」的落地：两个**反应性动作** + 两个瞬时标记。
//!
//! | 动作 | 消耗 | 效果 | 标记 |
//! | :--- | ---: | :--- | :--- |
//! | 翻滚 | 1 精力 | 远离威胁退一格 | [`Dodging`]（无敌帧 `DODGE_SECS`） |
//! | 招架 | 1 精力 | 挡下绑定的那次攻击并反制 | [`Parrying`]（绑实体 + 兜底过期） |
//!
//! 判定链：目标获取挂 `CollisionTarget` → 物理命中系统做防御判定
//! （[`resolve_defense`](crate::combat::formula::resolve_defense)）→ `apply_damage_system`
//! （唯一扣血点）。本域**不认识护甲**，伤害公式也**不认识防御**——
//! 两边在领域层用 [`DefenseState`](crate::combat::formula::DefenseState) 交接。

use crate::timeline::ActionOf;
use bevy::prelude::*;

use crate::combat::attack::abilities::PARRY_ABILITY;

pub mod actions;
pub mod components;
pub mod events;
pub mod stamina;
pub mod systems;

pub use actions::{
    DODGE_SECS, PARRY_COST, PARRY_SECS, PARRY_TIMING, ROLL_COST, ROLL_SPEED, declare_parry_system,
    declare_roll, declare_roll_system, parry_executor_system, roll_executor_system, roll_step,
};
pub use components::{BlockChance, DefenseOutcome, Dodging, ParryAction, Parrying};
pub use events::{ParryCommand, RollCommand};
pub use stamina::{STAMINA_REGEN_PER_DECISION, Stamina};
pub use systems::{expire_defense_markers_system, recover_stamina_observer};

/// 招架行动工厂：载荷 + 调度数据（抬手一挡，随手就能改主意，撤销免费）。
pub fn parry_action_scene(
    target_attack: Entity,
    timing: crate::timeline::ActionTiming,
    schedule: crate::timeline::ScheduledAction,
    actor: Entity,
) -> impl Scene {
    // 对抗标签（能不能被打断 / 招架 / 格挡）跟着载荷一起挂在行动实体上
    let tags = PARRY_ABILITY.combat;
    bsn! {
        template_value(tags)
        ActionOf({actor})
        ParryAction {
            target_attack: {target_attack},
        }
        template_value(timing)
        template_value(schedule)
    }
}

pub mod plugin;
pub use plugin::DefensePlugin;

/// 防御：翻滚 / 招架在本域系统链里的位置（跨子域的先后由 [`CombatPlugin`](super::CombatPlugin) 编排）。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DefenseSet;
