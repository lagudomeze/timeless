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

/// 四种判定结果（由 [`resolve_defense`](crate::combat::formula::resolve_defense) 给出）。
///
/// **不是 `Eq`**：`Blocked` 带着一个减伤比例（`f32`）。
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum DefenseOutcome {
    /// 命中
    #[default]
    Landed,
    /// 被无敌帧闪开
    Dodged,
    /// 被招架（免伤 + 反制）
    Parried,
    /// 被格挡（**减伤**，不是免伤）
    Blocked {
        /// 减伤比例（0..1）
        absorbed: f32,
    },
}

/// 格挡率（0.0..=1.0）：命中管线第 ③ 关读它。
///
/// **来源是装备 / 姿态**（见 `docs/combat.md` 第五节）——现在先作为单位身上的
/// 属性存在，等装备系统（M27）落地后改成从装备读。管线只读它、不自己算，
/// 所以这次改动不需要动管线。
///
/// **格挡没有标记组件**（对比 [`Dodging`] / [`Parrying`]）：那两者要跨帧存在
/// （无敌帧有时间窗、招架等的是一次具体攻击），而格挡在命中系统里**当帧就结算完了**，
/// 结果落在 [`DefenseOutcome::Blocked`] 上，没有"还在格挡"这种状态可表达。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct BlockChance(pub f32);

impl BlockChance {
    /// 夹在 0..=1：设计数据写错不该让伤害算成负数。
    pub fn clamped(self) -> f32 {
        self.0.clamp(0.0, 1.0)
    }
}
