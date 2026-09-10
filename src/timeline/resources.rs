//! 时间线状态资源。

use std::time::Duration;

use bevy::prelude::*;

/// 一个推进窗口的时长（虚拟秒）。
///
/// 窗口内所有已提交的行动按 `execute_at` 到点执行；窗口一过，世界重新冻结。
pub const RESOLUTION_WINDOW: f32 = 1.0;

/// 回合阶段。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// 规划：虚拟时间冻结，等玩家声明并提交
    #[default]
    Planning,
    /// 推进：虚拟时间流动，已提交的行动到点执行
    Resolving,
}

/// 时间线状态（整个游戏唯一的阶段真相）。
#[derive(Resource, Debug)]
pub struct Timeline {
    phase: Phase,
    round: u32,
    window: Timer,
}

impl Default for Timeline {
    fn default() -> Self {
        Self {
            phase: Phase::Planning,
            round: 0,
            window: Timer::from_seconds(RESOLUTION_WINDOW, TimerMode::Once),
        }
    }
}

impl Timeline {
    /// 当前阶段。
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// 是否在规划阶段（只有这时才接受新的行动声明）。
    pub fn is_planning(&self) -> bool {
        self.phase == Phase::Planning
    }

    /// 当前 / 刚结束的轮次编号（从 1 开始，未开打为 0）。
    pub fn round(&self) -> u32 {
        self.round
    }

    /// 进入推进阶段（提交），返回本轮编号；已在推进阶段则返回 `None`。
    pub fn begin_resolution(&mut self) -> Option<u32> {
        if !self.is_planning() {
            return None;
        }
        self.phase = Phase::Resolving;
        self.round += 1;
        self.window.reset();
        Some(self.round)
    }

    /// 推进窗口；返回窗口是否刚刚结束（只有推进阶段会结束）。
    pub fn tick_resolution(&mut self, delta: Duration) -> bool {
        if self.is_planning() {
            return false;
        }
        self.window.tick(delta).just_finished()
    }

    /// 推进阶段还剩多少秒（规划阶段返回 `None`）。
    pub fn window_remaining(&self) -> Option<f32> {
        if self.is_planning() {
            return None;
        }
        let remaining = self.window.duration().as_secs_f32() - self.window.elapsed_secs();
        Some(remaining.max(0.0))
    }

    /// 回到规划阶段，返回刚结束的轮次编号。
    pub fn end_round(&mut self) -> u32 {
        self.phase = Phase::Planning;
        self.window.reset();
        self.round
    }

    /// 重置（战斗重开）：回到第一轮之前的规划阶段。
    pub fn restart(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_starts_frozen_in_planning() {
        let timeline = Timeline::default();
        assert!(timeline.is_planning(), "开局应当是规划阶段（等玩家输入）");
        assert_eq!(timeline.round(), 0);
    }

    #[test]
    fn commit_opens_a_window_that_only_closes_once() {
        let mut timeline = Timeline::default();
        assert_eq!(timeline.begin_resolution(), Some(1), "提交应开始第 1 轮");
        assert_eq!(timeline.begin_resolution(), None, "推进中重复提交应被忽略");
        assert!(!timeline.tick_resolution(Duration::from_secs_f32(0.5)));
        assert!(timeline.tick_resolution(Duration::from_secs_f32(0.6)));
        assert_eq!(timeline.end_round(), 1);
        assert!(timeline.is_planning(), "窗口结束应回到规划阶段");
    }

    #[test]
    fn planning_phase_never_ticks_the_window() {
        let mut timeline = Timeline::default();
        assert!(!timeline.tick_resolution(Duration::from_secs(10)));
        assert_eq!(timeline.round(), 0);
    }

    #[test]
    fn restart_returns_to_the_first_round() {
        let mut timeline = Timeline::default();
        timeline.begin_resolution();
        timeline.restart();
        assert!(timeline.is_planning());
        assert_eq!(timeline.round(), 0);
    }
}
