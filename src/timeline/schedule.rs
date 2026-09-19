//! 行动实体的调度数据：**什么时候**执行，后摇多长，多难被打断。
//!
//! 调度器只认识这里的东西；「这行动是什么」由载荷组件决定（[`crate::movement::MoveAction`]、
//! [`crate::combat::skills::FireballAction`]、[`crate::combat::skills::MeleeAction`]…），
//! 调度器永远不读它们。
//!
//! **行动者不在这里**：行动实体是行动者的**子实体**（Bevy 的 `ChildOf` 关系），
//! 「这条行动是谁的」由父子关系直接回答。父节点被销毁时子实体跟着销毁（`Children`
//! 是 linked spawn），因此不存在"行动者死了、行动还在半空"这种孤儿状态。
//!
//! 无回合模型里没有「提交」这一步：声明时刻即前摇起点，
//! `execute_at = declared_at + windup`；执行器只看 `now >= execute_at`。
//! 行动者的三个阶段住在 [`DecisionSlot`](super::DecisionSlot) 里，这里只有时间戳。

use bevy::prelude::*;

use super::resources::Focus;
use super::timing::ActionTiming;

/// 行动实体的调度数据。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ScheduledAction {
    /// 声明时刻（虚拟秒）
    pub declared_at: f32,
    /// 执行时刻（虚拟秒）
    pub execute_at: f32,
    /// 后摇（虚拟秒）：执行时刻 + 后摇 = 重新可决策时刻
    pub recovery: f32,
    /// 打断抗性：被打断时掷骰防守方那一侧的底数
    pub interrupt_resist: i32,
}

impl Default for ScheduledAction {
    /// 只为满足 BSN 模板约束而存在；真实值一律用 [`ScheduledAction::declared_at`] 构造。
    fn default() -> Self {
        Self {
            declared_at: 0.0,
            execute_at: f32::INFINITY,
            recovery: 0.0,
            interrupt_resist: 0,
        }
    }
}

impl ScheduledAction {
    /// 声明：按「现在 + 前摇」定下执行时刻。
    pub fn declared_at(timing: ActionTiming, now: f32) -> Self {
        Self {
            declared_at: now,
            execute_at: now + timing.windup,
            recovery: timing.recovery,
            interrupt_resist: timing.interrupt_resist,
        }
    }

    /// 玩家（有 Focus 的一方）声明：想归零前摇且还有余量时，扣 1 点并把
    /// `execute_at` 定在**现在**。
    ///
    /// `execute_at = now` 的行动由**下一帧**的执行器处理（声明系统与执行器同帧，
    /// 执行器先跑）。这一帧延迟就是「瞬时生效」的全部代价：语义上它不算前摇，
    /// 因此谁也来不及在它落地前把它撤掉或打断。
    pub fn with_focus(
        timing: ActionTiming,
        now: f32,
        focus: &mut Focus,
        zero_windup: bool,
    ) -> Self {
        let mut schedule = Self::declared_at(timing, now);
        if zero_windup && focus.spend() {
            schedule = schedule.with_zero_windup();
        }
        schedule
    }

    /// 前摇时长（虚拟秒）：`execute_at - declared_at`。
    pub fn windup(&self) -> f32 {
        (self.execute_at - self.declared_at).max(0.0)
    }

    /// **前摇归零**：执行时刻就是声明时刻。
    ///
    /// [`ScheduledAction::with_focus`] 用它兑现 Focus；测试与「本来就该瞬发」的动作
    /// （比如脚本化的处决）也走同一条路，免得各处自己改 `execute_at`。
    pub fn with_zero_windup(mut self) -> Self {
        self.execute_at = self.declared_at;
        self
    }

    /// 这条行动占住行动者的总时长（前摇 + 后摇）。
    pub fn total(&self) -> f32 {
        self.windup() + self.recovery
    }

    /// 这条行动的「还没到点」吗（= 还能被撤销 / 打断）。
    pub fn pending(&self, now: f32) -> bool {
        now < self.execute_at
    }

    /// 该执行了吗（执行器的判据）。
    ///
    /// 用**严格大于**：声明系统与执行器在同一帧、且声明在前，因此
    /// `execute_at = now` 的零前摇行动会落到**下一帧**执行。这条一帧延迟换来两个性质：
    ///
    /// 1. 「用 Focus 抢先手」在语义上不像前摇（玩家感知为瞬时），但也不是同帧瞬移；
    /// 2. 反应系统能看见「玩家刚举起来的那一手」——否则零前摇的行动同帧消失，
    ///    威胁窗口会以为玩家还没表态，把世界继续冻着。
    pub fn due(&self, now: f32) -> bool {
        now > self.execute_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 调度器的测试不该依赖任何具体载荷：自己造一个节奏。
    const TEST_TIMING: ActionTiming = ActionTiming::new(0.2, 0.3, 3);

    #[test]
    fn schedule_derives_everything_from_the_declaration_time() {
        let schedule = ScheduledAction::declared_at(TEST_TIMING, 2.0);
        assert!(
            (schedule.windup() - TEST_TIMING.windup).abs() < 1e-5,
            "前摇 = execute_at - declared_at"
        );
        assert!((schedule.total() - TEST_TIMING.total()).abs() < 1e-5);
        assert_eq!(schedule.interrupt_resist, TEST_TIMING.interrupt_resist);
        assert!(schedule.pending(2.0), "刚声明时还在前摇");
        assert!(schedule.pending(2.19));
        assert!(!schedule.pending(2.20), "到点就不算 pending 了");
        assert!(!schedule.due(2.20), "到点那一帧还没轮到执行器");
        assert!(schedule.due(2.21), "过了执行时刻就该落地");
    }

    #[test]
    fn focus_zeroes_the_windup_and_spends_a_point() {
        let mut focus = Focus::default();
        let schedule = ScheduledAction::with_focus(TEST_TIMING, 5.0, &mut focus, true);
        assert_eq!(
            schedule.execute_at, 5.0,
            "用 Focus 换来的就是「现在就落地」"
        );
        assert_eq!(schedule.windup(), 0.0);
        assert_eq!(focus.current, crate::timeline::FOCUS_MAX - 1, "扣掉 1 点");
    }

    #[test]
    fn focus_is_not_spent_when_the_player_does_not_ask_for_it() {
        let mut focus = Focus::default();
        let schedule = ScheduledAction::with_focus(TEST_TIMING, 5.0, &mut focus, false);
        assert!((schedule.windup() - TEST_TIMING.windup).abs() < 1e-5);
        assert_eq!(focus.current, crate::timeline::FOCUS_MAX);
    }

    #[test]
    fn an_empty_focus_pool_falls_back_to_the_normal_windup() {
        let mut focus = Focus { current: 0, max: 3 };
        let schedule = ScheduledAction::with_focus(TEST_TIMING, 5.0, &mut focus, true);
        assert!(
            (schedule.windup() - TEST_TIMING.windup).abs() < 1e-5,
            "没有余量就只能排前摇"
        );
    }
}
