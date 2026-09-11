//! 行动实体的调度组件与状态标记。
//!
//! 调度器只认识这里的东西；「这行动是什么」由载荷组件决定（`MoveAction`、
//! `ShootAction`、`MeleeAction`…），调度器永远不读它们。

use bevy::prelude::*;

use super::timing::ActionTiming;

/// 行动实体的调度数据：**谁**在**什么时候**执行，节奏多长。
///
/// 无回合模型里没有「提交」这一步：声明时刻即前摇起点，
/// `execute_at = declared_at + timing.windup`。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ScheduledAction {
    /// 行动者（单位实体）
    pub actor: Entity,
    /// 固定节奏（前摇 + 后摇）
    pub timing: ActionTiming,
    /// 声明时刻（虚拟秒）
    pub declared_at: f32,
    /// 执行时刻（虚拟秒）
    pub execute_at: f32,
}

impl Default for ScheduledAction {
    /// 只为满足 BSN 模板约束而存在；真实值一律用 [`ScheduledAction::declared_at`] 构造。
    fn default() -> Self {
        Self {
            actor: Entity::PLACEHOLDER,
            timing: ActionTiming::new(0.0, 0.0),
            declared_at: 0.0,
            execute_at: f32::INFINITY,
        }
    }
}

impl ScheduledAction {
    /// 声明：按「现在 + 前摇」定下执行时刻。
    pub fn declared_at(actor: Entity, timing: ActionTiming, now: f32) -> Self {
        Self {
            actor,
            timing,
            declared_at: now,
            execute_at: now + timing.windup,
        }
    }

    /// 执行落地时给出的后摇窗口：`until` 必须严格大于落地时刻。
    ///
    /// 用「落地时刻 + 后摇」而不是「执行时刻 + 后摇」，这样即便后摇为 0，
    /// 恢复系统也不会在同一帧就把 `Ready` 加回来（避免同帧声明+执行+就绪的抖动）。
    pub fn recovery_window(&self, executed_at: f32) -> BusyRecovery {
        BusyRecovery {
            executed_at,
            ready_at: executed_at + self.timing.recovery,
        }
    }
}

/// 状态：**已声明**（`require_commit = true` 时的草案，可被新声明覆盖）。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Declared;

/// 状态：**已提交**（等待到点）。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Pending;

/// 状态：**已到点**（本帧等待执行器处理）。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Committed;

/// 单位「现在可以决策」。
///
/// 声明动作时移除，后摇结束时恢复；`require_commit = true` 且草案还在时也保持移除。
/// **这是无回合模型里唯一的「轮到谁」判据**——取代了旧的 `Phase::Planning`。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Ready;

/// 单位正在后摇：`until` 之前不接受新决策。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct BusyRecovery {
    /// 执行时刻（虚拟秒）
    pub executed_at: f32,
    /// 重新可决策时刻（虚拟秒）
    pub ready_at: f32,
}
