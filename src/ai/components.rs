//! 敌人行为组件。

use bevy::prelude::*;

/// 敌人决策参数。
///
/// 无回合模型下敌人靠自己的节奏决策：决策槽空闲
/// （[`DecisionSlot::is_idle`](crate::timeline::DecisionSlot::is_idle)）就选一个战术。
/// 「射程」不在这里——那是**武器**的属性（[`AttackRange`](crate::combat::AttackRange)）。
///
/// **`#[require(Tactic)]`**：两个 AI 系统都要求 `&mut Tactic`，少了它 AI 会**静默地
/// 一行都不执行**（敌人站着不动）。用 `require` 声明这条依赖，组装层就不可能再漏。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq)]
#[reflect(Component)]
#[require(Tactic)]
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

/// 敌人本次决策选出的**战术**（HUD / 死亡复盘读它）。
///
/// 战术只回答「打算干什么」，不回答「什么时候出手」——把它落到一手具体行动是下一层
/// （`Situation` → `Tactic` → `Intent` → 行动实体，见 `docs/timeline.md` 第二节）。
#[derive(Component, Reflect, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub enum Tactic {
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
