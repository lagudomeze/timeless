//! 战斗领域插件：**只编排子域**——注册哪些消息、跑哪些系统都是各子域自己的事
//! （每个子域一个 `plugin.rs`），这里只说"谁先谁后"。
//!
//! 顺序即语义（见模块文档的流水线图）：威胁先看 → 防御标记过期 → 声明 →
//! 执行器 / 投射物 → 目标获取 → 命中结算 → 扣血 → 清理。
//!
//! **子域之间靠 `SystemSet` 排序，不靠插件添加顺序**——Bevy 的 `Plugin` 添加顺序
//! 不决定系统顺序，那样写出来的"顺序"是假的（一改就散，而且测试可能照样过）。

use bevy::prelude::*;

use super::CombatSet;

use super::attack::{AttackPlugin, AttackSet};
use super::defense::{DefensePlugin, DefenseSet};
use super::formula::{FormulaPlugin, FormulaSet};
use super::health::{HealthPlugin, HealthSet};
use super::lifecycle::{LifecyclePlugin, LifecycleSet};
use super::reaction::{ReactionPlugin, ReactionSet};
use super::targeting::{TargetingPlugin, TargetingSet};

/// 战斗领域插件：只编排，不注册。
#[derive(Debug, Default)]
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ReactionPlugin,
            DefensePlugin,
            AttackPlugin,
            TargetingPlugin,
            FormulaPlugin,
            HealthPlugin,
            LifecyclePlugin,
        ))
        // 子域的先后就是这一行：每个子域把自己的系统放进自己的 `*Set`，
        // 这里按战斗流程串起来。子域**内部**的顺序由各自的 `plugin.rs` 维护。
        .configure_sets(
            Update,
            (
                ReactionSet,
                DefenseSet,
                AttackSet,
                TargetingSet,
                FormulaSet,
                HealthSet,
                LifecycleSet,
            )
                .chain()
                .in_set(CombatSet),
        );
    }
}
