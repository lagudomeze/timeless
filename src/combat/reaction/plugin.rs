//! 反应子域插件：威胁检测、表态消息与窗口推进。

use bevy::prelude::*;

use super::ReactionSet;

use super::{
    ReactionAnswer, detect_threat_system, mark_threatened_system, resolve_reaction_system,
};

/// 威胁检测 → 反应窗口 → 玩家表态。
pub struct ReactionPlugin;

impl Plugin for ReactionPlugin {
    fn build(&self, app: &mut App) {
        app
            // 玩家表态：写方是 input（技能键 / 右键），消费方是本域
            .add_message::<ReactionAnswer>()
            .add_systems(
                Update,
                (
                    // 表态**先落地**：玩家这一帧按下的反制，当帧就算"已表态"，世界当帧就能动
                    resolve_reaction_system,
                    // 威胁再看：有东西打向玩家就请求冻结（本帧末生效）
                    detect_threat_system,
                    // 打标记必须晚于检测：同帧写入会让检测自己漏检
                    mark_threatened_system,
                )
                    .chain()
                    .in_set(ReactionSet),
            );
    }
}
