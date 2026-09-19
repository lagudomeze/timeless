//! 战斗领域插件：注册消息与战斗流水线。
//!
//! 顺序即语义（见模块文档的流水线图）：威胁检测 → 声明 → 攻击实体 / 投射物
//! → 目标获取 → 命中结算（伤害 / 防御 / 打断）→ 扣血 → 生命周期清理。

use bevy::prelude::*;

use super::CombatSet;
use super::defense::{
    ParryCommand, RollCommand, declare_parry_system, declare_roll_system,
    expire_defense_markers_system, parry_executor_system, recover_stamina_observer,
    roll_executor_system,
};
use super::formula::{apply_physical_hits_system, interrupt_observer};
use super::health::{DamageEvent, DeathEvent, apply_damage_system, despawn_dead_system};
use super::lifecycle::{cleanup_finished_attacks_system, expire_attack_entities_system};
use super::reaction::{ThreatWindow, detect_threat_system};
use super::skills::{
    CycleSkill, FireCommand, MeleeCommand, MenuSelection, ProjectileArrived, SelectSkill,
    UseSelectedSkill, cycle_skill_system, declare_fireball_system, declare_melee_system,
    explosion_system, fireball_action_executor_system, melee_action_executor_system,
    projectile_arrival_system, refund_fireball_observer, refund_melee_observer,
    register_combat_abilities_system, select_skill_system, use_selected_skill_system,
};
use super::targeting::{detect_collisions_system, detect_melee_system};

/// 战斗领域插件。
///
/// 系统链顺序即战斗流水线；跨领域顺序由 [`GamePlugin`](crate::GamePlugin) 统一编排。
#[derive(Debug, Default)]
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        // 本域的定义是**技能目录里的条目**，所以目录必须存在：缺了就补上，
        // 否则「交上去」这一步无处可去（只装战斗域的单测也不会因此炸）。
        if !app.is_plugin_added::<crate::skills::SkillPlugin>() {
            app.add_plugins(crate::skills::SkillPlugin);
        }
        app.init_resource::<MenuSelection>()
            // 技能目录的静态数据在启动时交上去（数值仍归本域，见 `docs/skills.md`）
            .add_systems(Startup, register_combat_abilities_system)
            // 反应窗口状态：威胁何时出现、玩家表态了没有
            .init_resource::<ThreatWindow>()
            .add_message::<DeathEvent>()
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
            // EntityEvent 订阅：撤销退款（谁收钱谁退）+ 后摇结束回精力
            // 打断对抗：命中触发的战斗判定，落地在战斗域自己收
            .add_observer(interrupt_observer)
            .add_observer(refund_fireball_observer)
            .add_observer(refund_melee_observer)
            .add_observer(recover_stamina_observer)
            .add_systems(
                Update,
                (
                    // 威胁先看：有东西打向玩家就请求冻结（本帧末生效）
                    detect_threat_system,
                    // 防御标记先过期，本帧到期的无敌帧不该再生效
                    expire_defense_markers_system,
                    // 菜单先更新选择，再按选择派发成各领域的指令
                    (select_skill_system, cycle_skill_system),
                    use_selected_skill_system,
                    (
                        declare_fireball_system,
                        declare_melee_system,
                        declare_roll_system,
                        declare_parry_system,
                    ),
                    // 执行器到点落地（各自判断 execute_at 并自己收尾）
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
                    // 命中结算：防御判定 + 护甲 + 打断触发 + 命中计数
                    apply_physical_hits_system,
                    // 扣血 → 死亡消息；销毁放最后，其他系统这一帧还能读到尸体
                    apply_damage_system,
                    cleanup_finished_attacks_system,
                    expire_attack_entities_system,
                    despawn_dead_system,
                )
                    .chain()
                    .in_set(CombatSet),
            );
    }
}
