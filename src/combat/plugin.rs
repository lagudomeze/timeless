//! 战斗领域插件：注册消息与战斗流水线。

use bevy::prelude::*;

use super::CombatSet;
use super::formula::{DamageEvent, apply_physical_damage_system};
use super::health::{
    DeathEvent, ModifyHealthEvent, apply_damage, despawn_dead_system, request_damage_system,
};
use super::lifecycle::{
    cleanup_finished_attacks_system, expire_attack_entities_system, manage_projectile_hits_system,
};
use super::skills::{
    FireCommand, MeleeCommand, declare_skill_system, melee_action_executor_system,
    shoot_action_executor_system,
};
use super::targeting::{detect_collisions_system, detect_melee_system};

/// 战斗领域插件。
///
/// 系统链顺序即战斗流水线（见模块文档）；跨领域顺序由
/// [`GamePlugin`](crate::GamePlugin) 统一编排。
#[derive(Debug, Default)]
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ModifyHealthEvent>()
            .add_message::<DeathEvent>()
            .add_message::<DamageEvent>()
            .add_message::<FireCommand>()
            .add_message::<MeleeCommand>()
            .add_systems(
                Update,
                (
                    declare_skill_system,
                    shoot_action_executor_system,
                    melee_action_executor_system,
                    detect_collisions_system,
                    detect_melee_system,
                    apply_physical_damage_system,
                    manage_projectile_hits_system,
                    request_damage_system,
                    apply_damage,
                    despawn_dead_system,
                    cleanup_finished_attacks_system,
                    expire_attack_entities_system,
                )
                    .chain()
                    .in_set(CombatSet),
            );
    }
}
