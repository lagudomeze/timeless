//! 行动实体的调度组件与行动者的决策槽。
//!
//! 调度器只认识这里的东西；「这行动是什么」由载荷组件决定（`MoveAction`、
//! `FireballAction`、`MeleeAction`…），调度器永远不读它们。
//!
//! **没有状态标记**：一条行动的「前摇中 / 已到点 / 正在后摇」全部由
//! `ScheduledAction.execute_at` 与行动者身上的 [`DecisionSlot`] / [`Busy`] 推导——
//! 于是不存在「标记忘了摘」这类状态与时间戳打架的 bug。

use bevy::prelude::*;

use super::resources::Focus;
use super::timing::ActionTiming;

/// 行动实体的调度数据：**谁**在**什么时候**执行，后摇多长，多难被打断。
///
/// 无回合模型里没有「提交」这一步：声明时刻即前摇起点，
/// `execute_at = declared_at + windup`；执行器只看 `now >= execute_at`。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ScheduledAction {
    /// 行动者（单位实体）
    pub actor: Entity,
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
            actor: Entity::PLACEHOLDER,
            declared_at: 0.0,
            execute_at: f32::INFINITY,
            recovery: 0.0,
            interrupt_resist: 0,
        }
    }
}

impl ScheduledAction {
    /// 声明：按「现在 + 前摇」定下执行时刻。
    pub fn declared_at(actor: Entity, timing: ActionTiming, now: f32) -> Self {
        Self {
            actor,
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
        actor: Entity,
        timing: ActionTiming,
        now: f32,
        focus: &mut Focus,
        zero_windup: bool,
    ) -> Self {
        let mut schedule = Self::declared_at(actor, timing, now);
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

/// 行动者的**决策槽**：`Empty` = 现在可以声明行动，`Filled` = 忙。
///
/// 阶段不再靠槽状态区分，而是靠「有没有行动实体 + 有没有后摇」：
///
/// | 状态 | 判据 |
/// | :--- | :--- |
/// | 前摇 | `Filled` 且有 `ScheduledAction.actor == 自己` 的行动实体 |
/// | 后摇 | `Filled` 且没有行动实体、但有 [`Busy`] |
/// | 空闲 | `Empty` |
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DecisionSlot {
    /// 空闲：可以声明行动
    #[default]
    Empty,
    /// 忙碌：前摇中或后摇中
    Filled,
}

/// 后摇：`until`（虚拟秒）之前不接受新决策。
///
/// 由**执行器**在收尾时挂上，由 [`recovery_system`](crate::timeline::recovery_system)
/// 到点摘掉并把决策槽清空。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Busy {
    /// 重新可决策时刻（虚拟秒）
    pub until: f32,
}

impl Busy {
    /// 收尾窗口：后摇从**效果落地那一刻**起算，但至少忙到 `busy_until`。
    ///
    /// 带位移 / 飞行的动作必须把「效果真的发生」的那一刻传进来（移动走到格中心、
    /// 火球飞到落点），否则行动者会在效果还在进行时拿到决策权。
    pub fn after(schedule: &ScheduledAction, executed_at: f32, busy_until: f32) -> Self {
        Self {
            until: (executed_at + schedule.recovery).max(busy_until),
        }
    }
}

/// 行动的可取消规则（挂在行动实体上，替代旧的 `ActionCost` /
/// `CancelCost` / `Uncancellable` 三件套）。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Cancellable {
    /// 免费撤销：还退什么也没花，收了也没扣
    #[default]
    Free,
    /// 撤销要退 `refund`、再扣 `penalty`（声明时就扣了资源的动作）
    Cost { refund: u32, penalty: u32 },
    /// 根本不给撤（跳跃那种「起跳就谁都别想插队」）
    Never,
}

impl Cancellable {
    /// 撤销时退还多少资源。
    pub fn refund(self) -> u32 {
        match self {
            Self::Cost { refund, .. } => refund,
            Self::Free | Self::Never => 0,
        }
    }

    /// 撤销本身要付多少代价。
    pub fn penalty(self) -> u32 {
        match self {
            Self::Cost { penalty, .. } => penalty,
            Self::Free | Self::Never => 0,
        }
    }
}

/// 「这个单位的决策来自玩家输入」。
///
/// 时间线（等谁决策、冻结世界）与反应系统（谁被威胁）都只认这个标记，
/// 不再到处 `find(|faction| faction == Faction::Player)`：
/// `Faction` 管**战斗目标过滤**，`InputDriven` 管**输入归属**，两者语义不同。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct InputDriven;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::timing;

    #[test]
    fn schedule_derives_everything_from_the_declaration_time() {
        let schedule = ScheduledAction::declared_at(Entity::PLACEHOLDER, timing::MELEE, 2.0);
        assert!(
            (schedule.windup() - timing::MELEE.windup).abs() < 1e-5,
            "前摇 = execute_at - declared_at"
        );
        assert!((schedule.total() - timing::MELEE.total()).abs() < 1e-5);
        assert_eq!(schedule.interrupt_resist, timing::MELEE.interrupt_resist);
        assert!(schedule.pending(2.0), "刚声明时还在前摇");
        assert!(schedule.pending(2.19));
        assert!(!schedule.pending(2.20), "到点就不算 pending 了");
        assert!(!schedule.due(2.20), "到点那一帧还没轮到执行器");
        assert!(schedule.due(2.21), "过了执行时刻就该落地");
    }

    #[test]
    fn focus_zeroes_the_windup_and_spends_a_point() {
        let mut focus = Focus::default();
        let schedule =
            ScheduledAction::with_focus(Entity::PLACEHOLDER, timing::SHOOT, 5.0, &mut focus, true);
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
        let schedule =
            ScheduledAction::with_focus(Entity::PLACEHOLDER, timing::SHOOT, 5.0, &mut focus, false);
        assert!((schedule.windup() - timing::SHOOT.windup).abs() < 1e-5);
        assert_eq!(focus.current, crate::timeline::FOCUS_MAX);
    }

    #[test]
    fn an_empty_focus_pool_falls_back_to_the_normal_windup() {
        let mut focus = Focus { current: 0, max: 3 };
        let schedule =
            ScheduledAction::with_focus(Entity::PLACEHOLDER, timing::SHOOT, 5.0, &mut focus, true);
        assert!(
            (schedule.windup() - timing::SHOOT.windup).abs() < 1e-5,
            "没有余量就只能排前摇"
        );
    }

    #[test]
    fn busy_window_starts_at_the_effect_not_at_the_schedule() {
        let schedule = ScheduledAction::declared_at(Entity::PLACEHOLDER, timing::MOVE, 0.0);
        let busy = Busy::after(&schedule, 1.0, 1.4);
        assert_eq!(
            busy.until, 1.4,
            "忙碌窗口取「后摇」与「效果落地」里更晚的那个"
        );

        let busy = Busy::after(&schedule, 1.0, 1.0);
        assert_eq!(
            busy.until,
            1.0 + timing::MOVE.recovery,
            "效果瞬间完成的动作（近战）只忙一个后摇"
        );
    }

    #[test]
    fn cancellable_carries_its_own_refund_and_penalty() {
        assert_eq!(Cancellable::Free.refund(), 0);
        assert_eq!(Cancellable::Never.penalty(), 0);
        let cost = Cancellable::Cost {
            refund: 2,
            penalty: 1,
        };
        assert_eq!((cost.refund(), cost.penalty()), (2, 1));
    }
}
