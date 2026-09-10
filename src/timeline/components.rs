//! 行动实体的调度组件与状态标记。
//!
//! 调度器只认识这里的东西；「这行动是什么」由载荷组件决定（`MoveAction`、
//! `ShootAction`、`MeleeAction`…），调度器永远不读它们。

use bevy::prelude::*;

/// 行动实体的调度数据：**谁**在**什么时候**执行，前摇多长。
///
/// 载荷（动作内容）挂在同一实体的其它组件上——[`crate::timeline`] 因此不必知道
/// 这是移动、射击还是将来的任何新动作。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ScheduledAction {
    /// 行动者（单位实体）
    pub actor: Entity,
    /// 前摇（虚拟秒）：提交时刻 + 前摇 = 执行时刻
    pub windup: f32,
    /// 执行时刻（虚拟秒）：声明时先给草案值，提交时会按提交时刻重算
    pub execute_at: f32,
}

impl Default for ScheduledAction {
    /// 只为满足 BSN 模板约束而存在；真实值一律用 [`ScheduledAction::draft`] 构造。
    fn default() -> Self {
        Self {
            actor: Entity::PLACEHOLDER,
            windup: 0.0,
            execute_at: f32::INFINITY,
        }
    }
}

impl ScheduledAction {
    /// 规划阶段的草案：执行时刻留空，提交时由 [`commit_actions_system`] 写入。
    ///
    /// [`commit_actions_system`]: super::systems::commit_actions_system
    pub fn draft(actor: Entity, windup: f32) -> Self {
        Self {
            actor,
            windup,
            execute_at: f32::INFINITY,
        }
    }

    /// 提交：按「现在 + 前摇」定下执行时刻。
    pub fn committed_at(&self, now: f32) -> Self {
        Self {
            execute_at: now + self.windup,
            ..*self
        }
    }
}

/// 状态：**已声明**（规划阶段的草案，可被新声明覆盖）。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Declared;

/// 状态：**已提交**（推进阶段，等待到点）。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Pending;

/// 状态：**已到点**（本帧等待执行器处理）。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Committed;
