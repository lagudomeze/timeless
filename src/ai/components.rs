//! 敌人行为组件。

use bevy::prelude::*;

/// 敌人决策参数。
///
/// 无回合模型下敌人靠自己的节奏决策：一有 [`Ready`](crate::timeline::Ready) 就选一个意图。
/// 「射程」不在这里——那是**武器**的属性（[`AttackRange`](crate::combat::AttackRange)）。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct EnemyBrain {
    /// 超过这个距离（世界单位）就只想靠近
    pub engage_range: f32,
    /// 血量低于最大值的这个比例时转为保守（先退开一格）
    pub cautious_health_ratio: f32,
}

impl Default for EnemyBrain {
    fn default() -> Self {
        Self {
            engage_range: 12.0,
            cautious_health_ratio: 0.35,
        }
    }
}

/// 敌人本次决策选出的意图（HUD / 死亡复盘读它）。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// 目标太远：按兵不动
    #[default]
    Idle,
    /// 走近目标一格
    Approach,
    /// 贴脸：横扫
    Melee,
    /// 射程够但还没贴脸：扔火球（锁格 AoE）
    Shoot,
    /// 血量危险：退开一格
    Retreat,
    /// 有攻击正在前摇指向自己：翻滚闪开（威胁优先于贪刀）
    Dodge,
}
