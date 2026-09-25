//! 攻击子域插件：注册攻击行动的消息、菜单资源、退款观察者，并声明声明 → 执行 → 结算的顺序。

use bevy::prelude::*;

use super::AttackSet;

use super::{
    CycleSkill, FireCommand, MeleeCommand, MenuSelection, ProjectileArrived, SelectSkill,
    ShootCommand, UseSelectedSkill, cycle_skill_system, declare_fireball_system,
    declare_melee_system, declare_shoot_system, explosion_system, fireball_action_executor_system,
    melee_action_executor_system, projectile_arrival_system, refund_fireball_observer,
    refund_melee_observer, register_combat_abilities_system, select_skill_system,
    shoot_action_executor_system, use_selected_skill_system,
};

/// 攻击行动（火球 / 横扫 / 箭矢 + 爆炸）与技能菜单。
pub struct AttackPlugin;

impl Plugin for AttackPlugin {
    fn build(&self, app: &mut App) {
        // 本域的定义是**技能目录里的条目**，所以目录必须存在（只装本域的单测也要能跑）
        if !app.is_plugin_added::<crate::skills::SkillPlugin>() {
            app.add_plugins(crate::skills::SkillPlugin);
        }
        app.init_resource::<MenuSelection>()
            .add_systems(Startup, register_combat_abilities_system)
            .add_message::<ProjectileArrived>()
            // 输入类消息：由消费它们的领域注册（写：input；消费：本域）
            .add_message::<FireCommand>()
            .add_message::<MeleeCommand>()
            .add_message::<ShootCommand>()
            .add_message::<SelectSkill>()
            .add_message::<CycleSkill>()
            .add_message::<UseSelectedSkill>()
            // 撤销退款：谁收钱谁退
            .add_observer(refund_fireball_observer)
            .add_observer(refund_melee_observer)
            .add_systems(
                Update,
                (
                    // 菜单先更新选择，再按选择派发成各领域的指令
                    (select_skill_system, cycle_skill_system),
                    use_selected_skill_system,
                    (
                        declare_fireball_system,
                        declare_melee_system,
                        declare_shoot_system,
                    ),
                    // 执行器到点落地（各自判断 execute_at 并自己收尾）
                    (
                        melee_action_executor_system,
                        fireball_action_executor_system,
                        shoot_action_executor_system,
                    ),
                    // 投射物飞行：到格就炸（本帧到达本帧结算）
                    (projectile_arrival_system, explosion_system),
                    // 弹药缓慢回复（远程 / 重击那条线）：虚拟时间，冻结时不回
                    super::ammo::recover_ammo_system,
                )
                    .chain()
                    .in_set(AttackSet),
            );
    }
}
