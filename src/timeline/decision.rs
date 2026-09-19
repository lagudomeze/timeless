//! 行动者的**决策槽**：谁能声明行动，由它一个人说了算。
//!
//! 三个阶段**直接写在槽里**，而不是靠「有没有行动实体 / 有没有后摇标记」推导：
//!
//! | 状态 | 含义 | 行动实体 | 可撤销 | 可打断 |
//! | :--- | :--- | :--- | :--- | :--- |
//! | `Empty` | 空闲，可以声明 | 无 | — | — |
//! | `Windup` | 前摇中 | 有 | ✓ | ✓ |
//! | `Recovery { until }` | 后摇中 | 无 | ✗ | ✗ |
//!
//! 转换只有五条路，每条都只有一个作者：
//!
//! ```text
//! Empty ──声明（8 个声明系统）──▶ Windup ──执行器收尾──▶ Recovery { until }
//!   ▲                              │                        │
//!   │                              ├── 撤销（undo_system）──┤
//!   │                              └── 打断（Observer）─────┤
//!   └──────────────── recovery_system（now >= until）───────┘
//! ```
//!
//! 「谁在写槽」因此是穷举的、可审计的；不会出现「标记忘了摘」这类
//! 状态与时间戳打架的 bug。

use bevy::prelude::*;

use super::schedule::ScheduledAction;

/// 行动者的决策槽状态机。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub enum DecisionSlot {
    /// 空闲：可以声明行动
    #[default]
    Empty,
    /// 前摇中：行动实体还活着，随时可以反悔（撤销 / 打断）
    Windup,
    /// 后摇中：`until`（虚拟秒）之前不接受新决策
    Recovery {
        /// 重新可决策时刻（虚拟秒）
        until: f32,
    },
}

impl DecisionSlot {
    /// 现在能不能声明行动。
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// 执行器收尾：进入后摇。
    ///
    /// 后摇从**效果真的发生**那一刻起算，因此带位移 / 飞行的动作要把
    /// 「效果还要多久才发生」传进来（移动走到格中心、火球飞到落点）；
    /// 瞬间完成的动作传 `0`，只忙一个后摇。
    ///
    /// 传时长而不是「忙到哪个时刻」，是因为忙到的那一刻永远是
    /// `now + 这段时长`——少一次加法，也少一个"现在几点"的重复概念。
    pub fn recovering(schedule: &ScheduledAction, now: f32, effect_delay: f32) -> Self {
        Self::Recovery {
            until: now + schedule.recovery.max(effect_delay),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::timing;

    #[test]
    fn a_fresh_slot_is_empty() {
        assert_eq!(DecisionSlot::default(), DecisionSlot::Empty);
        assert!(DecisionSlot::Empty.is_empty());
        assert!(!DecisionSlot::Windup.is_empty());
        assert!(!DecisionSlot::Recovery { until: 1.0 }.is_empty());
    }

    /// 后摇取「一个后摇」与「效果还要多久」里更晚的那个。
    #[test]
    fn the_recovery_window_ends_at_the_later_of_effect_and_recovery() {
        let schedule = ScheduledAction::declared_at(Entity::PLACEHOLDER, timing::MOVE, 0.0);

        // 效果比后摇晚（移动 / 火球）：忙到效果真的发生
        assert_eq!(
            DecisionSlot::recovering(&schedule, 1.0, 0.4),
            DecisionSlot::Recovery { until: 1.4 }
        );
        // 效果瞬间完成（近战 / 招架）：只忙一个后摇
        assert_eq!(
            DecisionSlot::recovering(&schedule, 1.0, 0.0),
            DecisionSlot::Recovery {
                until: 1.0 + timing::MOVE.recovery
            }
        );
    }
}
