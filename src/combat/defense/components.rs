//! 防御组件：瞬时标记与结算结果。
//!
//! 载荷不在这里：翻滚是移动原语，其载荷 [`RollAction`](crate::movement::RollAction)
//! 住在移动领域；招架载荷 [`ParryAction`] 只绑实体、不产生位移，因此留在这里。

use bevy::prelude::*;

/// 招架载荷：挡下指定的这次攻击。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParryAction {
    /// 被招架的攻击实体
    pub target_attack: Entity,
}

impl Default for ParryAction {
    /// 只为满足 BSN 模板约束（`Default`）；真实值一律用
    /// [`parry_action_scene`](super::parry_action_scene) 构造。
    fn default() -> Self {
        Self {
            target_attack: Entity::PLACEHOLDER,
        }
    }
}

/// 翻滚后的无敌帧标记：`expires_at`（虚拟秒）之后失效。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Dodging {
    pub expires_at: f32,
}

/// 招架标记：绑定被招架的攻击实体，并带一个兜底过期时间。
///
/// 比只绑实体更稳：目标攻击被取消 / 销毁后，标记也会自动过期，
/// 不会留下永远不生效的幽灵状态。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Parrying {
    pub target_attack: Entity,
    pub expires_at: f32,
}

/// 一次攻击的判定结果（写：防御结算；消费：战斗日志 / 表现）。
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct AttackResolved {
    /// 攻击实体（近战横扫 / 箭矢）
    pub attacker: Entity,
    /// 被打的目标
    pub target: Entity,
    pub outcome: DefenseOutcome,
    /// 若被招架，攻击者应承受的反制伤害
    pub counter: f32,
}

/// 三种判定结果。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DefenseOutcome {
    /// 命中
    #[default]
    Landed,
    /// 被无敌帧闪开
    Dodged,
    /// 被招架（免伤 + 反制）
    Parried,
    /// 被**破势打断**：防御没挡住，但这一击没打出去（三层裁决 L3 的产物）
    Interrupted,
}

impl DefenseOutcome {
    /// 英文标签（HUD / 日志）。
    pub fn label(self) -> &'static str {
        match self {
            Self::Landed => "hit",
            Self::Dodged => "dodged",
            Self::Parried => "parried",
            Self::Interrupted => "interrupted",
        }
    }
}
