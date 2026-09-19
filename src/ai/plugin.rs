//! AI 领域插件。

use bevy::prelude::*;

use super::AiSet;
use super::systems::{decide_tactic_system, enemy_declare_system};

/// 敌人行为插件。
///
/// 没有独立的冷却系统：敌人「多久能再决策」由它上一个动作的后摇决定，
/// 时间线的 `recovery_system` 清空决策槽之后，本系统立刻再决策。
///
/// 两个系统链式执行：先**选战术**（HUD 能提前读到），再**声明行动**。
#[derive(Debug, Default)]
pub struct AiPlugin;

impl Plugin for AiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (decide_tactic_system, enemy_declare_system)
                .chain()
                .in_set(AiSet),
        );
    }
}
