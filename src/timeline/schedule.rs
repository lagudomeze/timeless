//! 行动实体身上**与时间有关的数据**：
//!
//! - [`ScheduledAction`]：**这一手什么时候落地**（声明时算出来的时间戳）；
//! - [`ActionTiming`]：**这类动作的节奏**（前摇 / 后摇 / 打断抗性），值归各领域；
//! - [`Uncancellable`]：**能不能撤**这一条规则标记。
//!
//! 调度器只认识这里的东西；「这行动是什么」由载荷组件决定（[`crate::movement::MoveAction`]、
//! [`crate::combat::skills::FireballAction`]、[`crate::combat::skills::MeleeAction`]…），
//! 调度器永远不读它们。行动者的三阶段在 [`DecisionSlot`](super::DecisionSlot)（那是
//! [`decision`](super::decision) 的事）；「这行动是谁的」由 [`ActionOf`](super::ActionOf)
//! 回答，「谁能撤它」由 [`Uncancellable`] 回答——这里放的是几个方面都要读的**规则与数据**。
//!
//! **行动者不在这里**：归属是独立的关系组件 [`ActionOf`](super::ActionOf) /
//! [`Actions`](super::Actions)（见 [`ownership`](super::ownership)），不是 `ChildOf`：
//! 行动实体没有 `Transform`，归属是纯逻辑。`Actions` 标了 `linked_spawn`，行动者被
//! 销毁时名下行动跟着销毁，因此不存在"行动者死了、行动还在半空"这种孤儿状态。
//!
//! **节奏是另一个组件**：前摇 / 后摇住在行动实体自己的 [`ActionTiming`] 组件上
//! （它是载荷的一部分），需要它的地方（执行器算忙碌窗口、HUD 画时间轴色块）直接读
//! 那个组件。[`ScheduledAction`] 只留这一手**声明时算出来**的两个数：什么时候落地、
//! 多难被打断。
//!
//! 无回合模型里没有「提交」这一步：声明时刻即前摇起点，
//! `execute_at = 声明时刻 + windup`；执行器只看 `now >= execute_at`。
//! 行动者的三个阶段住在 [`DecisionSlot`](super::DecisionSlot) 里。

use bevy::prelude::*;

use super::focus::Focus;

/// 一条行动的调度状态。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ScheduledAction {
    /// 执行时刻（虚拟秒）
    pub execute_at: f32,
}

impl Default for ScheduledAction {
    /// 只为满足 BSN 模板约束而存在；真实值一律用 [`ScheduledAction::declared_at`] 构造。
    fn default() -> Self {
        Self {
            execute_at: f32::INFINITY,
        }
    }
}

impl ScheduledAction {
    /// 声明：按「现在 + 前摇」定下执行时刻。
    pub fn declared_at(timing: ActionTiming, now: f32) -> Self {
        Self {
            execute_at: now + timing.windup,
        }
    }

    /// **前摇归零**：执行时刻就是现在。
    ///
    /// 玩家用 Focus 抢先手走这条路；测试与「本来就该瞬发」的动作（比如脚本化的处决）
    /// 也走它，免得各处自己改 `execute_at`。
    pub fn immediate(now: f32) -> Self {
        Self { execute_at: now }
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
        if zero_windup && focus.spend() {
            Self::immediate(now)
        } else {
            Self::declared_at(timing, now)
        }
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

/// 单个动作的固定节奏 + 打断抗性。
///
/// 没有状态机：`windup` 决定「什么时候到点」，`recovery` 决定「忙到什么时候」，
/// `interrupt_resist` 决定被打断时掷骰防守方那一侧的底数
/// （见 [`InterruptEvent`](crate::timeline::InterruptEvent)）。
///
/// **它描述的是载荷，不是调度器**：具体值归各领域（`movement` 的移动 / 跳跃 / 翻滚、
/// `combat::skills` 的近战 / 火球 / 箭矢、`combat::defense` 的招架）。
///
/// 声明时它被挂在**行动实体**上（和载荷一起，由场景工厂负责），于是：
/// 执行器算忙碌窗口、HUD 画时间轴色块都从这里读，不用在别处再抄一份；
/// 而 [`ScheduledAction`] 只剩这一手自己的时间戳。
///
/// **动作节奏的契约**：时间线只认这个形状，而 `ActionTiming` 这一族**只有类型，
/// 没有数值**。windup / recovery / interrupt_resist 是**载荷自己的属性**，因此具体值
/// （`MOVE_TIMING`、`FIREBALL_TIMING`…）住在各自的领域里，和载荷类型放在一起——
/// 这样新增一个动作时，时间线一行都不用改。
///
/// 数值后续外置成 `.ron`（见 [TODO.md](../../../TODO.md)），届时每个领域的常量
/// 换成从配置读，`ActionTiming` 的形状不变。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct ActionTiming {
    /// 前摇（虚拟秒）：声明时刻 + 前摇 = 执行时刻。
    pub windup: f32,
    /// 后摇（虚拟秒）：执行时刻 + 后摇 = 重新可决策时刻。
    pub recovery: f32,
    /// 打断抗性：掷骰对抗时加在防守方那一侧（越大越难被打断）。
    pub interrupt_resist: i32,
}

impl ActionTiming {
    /// 常量构造（`const` 便于各领域直接写常量表）。
    pub const fn new(windup: f32, recovery: f32, interrupt_resist: i32) -> Self {
        Self {
            windup,
            recovery,
            interrupt_resist,
        }
    }

    /// 从声明到重新可决策的总时长。
    pub fn total(&self) -> f32 {
        self.windup + self.recovery
    }
}

/// 「这条行动不给撤」。
///
/// 撤销的**代价**不在这里：花了什么、退多少、收多少手续费，都由花钱的那个领域
/// 订阅 [`ActionCancelled`](super::ActionCancelled) 自己算——行动实体上只留
/// "能不能撤"这一条规则。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Uncancellable;

#[cfg(test)]
mod tests {
    use super::*;

    /// 调度器的测试不该依赖任何具体载荷：自己造一个节奏。
    const TEST_TIMING: ActionTiming = ActionTiming::new(0.2, 0.3, 3);

    #[test]
    fn schedule_derives_everything_from_the_declaration_time() {
        let schedule = ScheduledAction::declared_at(TEST_TIMING, 2.0);
        assert_eq!(schedule.execute_at, 2.2, "执行时刻 = 声明时刻 + 前摇");
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
        assert_eq!(focus.current, crate::timeline::FOCUS_MAX - 1, "扣掉 1 点");
    }

    #[test]
    fn focus_is_not_spent_when_the_player_does_not_ask_for_it() {
        let mut focus = Focus::default();
        let schedule = ScheduledAction::with_focus(TEST_TIMING, 5.0, &mut focus, false);
        assert_eq!(schedule.execute_at, 5.0 + TEST_TIMING.windup);
        assert_eq!(focus.current, crate::timeline::FOCUS_MAX);
    }

    #[test]
    fn an_empty_focus_pool_falls_back_to_the_normal_windup() {
        let mut focus = Focus { current: 0, max: 3 };
        let schedule = ScheduledAction::with_focus(TEST_TIMING, 5.0, &mut focus, true);
        assert_eq!(
            schedule.execute_at,
            5.0 + TEST_TIMING.windup,
            "没有余量就只能排前摇"
        );
    }
}
