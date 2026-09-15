//! 防御组件：招架载荷 + 两个短命标记。
//!
//! 翻滚的载荷不在这里：它是移动原语，载荷 [`RollAction`](crate::movement::RollAction)
//! 住在移动领域；这里只有招架（它不产生位移）。

use bevy::prelude::*;

/// 招架载荷：挡下指定的**那一次攻击**（攻击实体，来自目标获取）。
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

/// 三种判定结果（由 [`resolve_defense`](crate::combat::formula::resolve_defense) 给出）。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DefenseOutcome {
    /// 命中
    #[default]
    Landed,
    /// 被无敌帧闪开
    Dodged,
    /// 被招架（免伤 + 反制）
    Parried,
}
