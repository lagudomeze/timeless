//! 战斗领域插件：注册消息与战斗流水线。
//!
//! 顺序即语义（见模块文档的流水线图）：
//! 声明 → 攻击实体生成 → 投射物到达/爆炸 → 目标获取 → **两阶段结算** → 扣血
//! → 生命周期清理。

use bevy::prelude::*;

use super::CombatSet;
use super::defense::{
    AttackResolved, ParryCommand, RollCommand, declare_parry_system, declare_roll_system,
    expire_defense_markers_system, parry_executor_system, refund_cancelled_actions_system,
    roll_executor_system,
};
use super::formula::{Arbitration, DamageEvent, phase1_arbitrate_system, phase2_apply_system};
use super::health::{
    DeathEvent, ModifyHealthEvent, apply_damage, despawn_dead_system, request_damage_system,
};
use super::lifecycle::{cleanup_finished_attacks_system, expire_attack_entities_system};
use super::skills::{
    CycleSkill, FireCommand, MeleeCommand, MenuSelection, ProjectileArrived, SelectSkill,
    UseSelectedSkill, cycle_skill_system, declare_fireball_system, declare_melee_system,
    explosion_system, fireball_action_executor_system, melee_action_executor_system,
    projectile_arrival_system, select_skill_system, use_selected_skill_system,
};
use super::targeting::{detect_collisions_system, detect_melee_system};

/// 战斗领域插件。
///
/// 系统链顺序即战斗流水线；跨领域顺序由 [`GamePlugin`](crate::GamePlugin) 统一编排。
#[derive(Debug, Default)]
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuSelection>()
            .init_resource::<Arbitration>()
            .add_message::<ModifyHealthEvent>()
            .add_message::<DeathEvent>()
            .add_message::<AttackResolved>()
            .add_message::<ProjectileArrived>()
            // 输入类消息：由消费它们的领域注册（写：input；消费：本域）
            .add_message::<RollCommand>()
            .add_message::<ParryCommand>()
            .add_message::<DamageEvent>()
            .add_message::<FireCommand>()
            .add_message::<MeleeCommand>()
            // 技能菜单：选择 / 循环 / 释放
            .add_message::<SelectSkill>()
            .add_message::<CycleSkill>()
            .add_message::<UseSelectedSkill>()
            .add_systems(
                Update,
                (
                    // 防御标记先过期，本帧到期的无敌帧不该再生效
                    expire_defense_markers_system,
                    // 撤销的退款：本帧退掉，别让玩家先看到扣费又看到退还
                    refund_cancelled_actions_system,
                    // 菜单先更新选择，再按选择派发成各领域的指令
                    (select_skill_system, cycle_skill_system),
                    use_selected_skill_system,
                    (
                        declare_fireball_system,
                        declare_melee_system,
                        declare_roll_system,
                        declare_parry_system,
                    ),
                    (
                        melee_action_executor_system,
                        fireball_action_executor_system,
                        roll_executor_system,
                        parry_executor_system,
                    ),
                    // 投射物飞行：到格就炸（本帧到达本帧结算）
                    (projectile_arrival_system, explosion_system),
                    detect_collisions_system,
                    detect_melee_system,
                    // 两阶段结算：阶段 1 只读裁决（快照一致），阶段 2 统一落地。
                    // 显式 `.pipe(IntoSystem::into_system)`：元组里混入签名不是 SystemParam 的
                    // 普通函数（如 `&mut Commands`）时，`IntoScheduleConfigs` 会**静默忽略**它。
                    // 两阶段结算：阶段 1 只读裁决（快照一致），阶段 2 统一落地。
                    // 两者通过 `Arbitration` 资源交接（不能用各自的 `Local`，那样缓冲不共享）。
                    phase1_arbitrate_system,
                    phase2_apply_system,
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
