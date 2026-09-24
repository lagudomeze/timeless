//! 生命子域插件：伤害 / 死亡消息与扣血、销毁系统。

use bevy::prelude::*;

use super::HealthSet;

use super::{DamageEvent, DeathEvent, apply_damage_system, despawn_dead_system};

/// 谁还有多少血、什么时候死。
pub struct HealthPlugin;

impl Plugin for HealthPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<DeathEvent>()
            .add_message::<DamageEvent>()
            // BRP 诊断锚点：血量是"为什么没死/伤害不对"第一个要看的数
            .register_type::<super::Health>()
            // 反射：BRP 要能直接读血量（排查"为什么没死/伤害不对"时最想看的数）
            .add_systems(
                Update, // 扣血 → 死亡消息；销毁放最后，其他系统这一帧还能读到尸体
                (apply_damage_system, despawn_dead_system)
                    .chain()
                    .in_set(HealthSet),
            );
    }
}
