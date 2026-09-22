//! 防御子域插件：翻滚 / 招架的消息、精力回复观察者，以及声明 → 执行的顺序。

use bevy::prelude::*;

use super::DefenseSet;

use super::{
    ParryCommand, RollCommand, declare_parry_system, declare_roll_system,
    expire_defense_markers_system, parry_executor_system, recover_stamina_observer,
    roll_executor_system,
};

/// 防御行动：翻滚（无敌帧）/ 招架（免伤 + 反制）。
pub struct DefensePlugin;

impl Plugin for DefensePlugin {
    fn build(&self, app: &mut App) {
        app
            // 输入类消息（写：input；消费：本域）
            .add_message::<RollCommand>()
            .add_message::<ParryCommand>()
            // 后摇结束回 1 点精力：时间线只宣布"他能决策了"，回多少归本域
            .add_observer(recover_stamina_observer)
            .add_systems(
                Update,
                (
                    // 防御标记先过期，本帧到期的无敌帧不该再生效
                    expire_defense_markers_system,
                    (declare_roll_system, declare_parry_system),
                    (roll_executor_system, parry_executor_system),
                )
                    .chain()
                    .in_set(DefenseSet),
            );
    }
}
