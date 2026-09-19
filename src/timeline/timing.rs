//! 动作节奏的**契约**：`ActionTiming` 的形状。
//!
//! 本模块只有类型，**没有数值**。原因见 [`ActionTiming`]：windup / recovery /
//! interrupt_resist 是**载荷自己的属性**，因此具体值（`MOVE_TIMING`、
//! `FIREBALL_TIMING`…）住在各自的领域里，和载荷类型放在一起——
//! 这样新增一个动作时，时间线一行都不用改。
//!
//! 数值后续外置成 `.ron`（见 [TODO.md](../../../TODO.md)），届时每个领域的常量
//! 换成从配置读，`ActionTiming` 的形状不变。

/// 单个动作的固定节奏 + 打断抗性。
///
/// 没有状态机：`windup` 决定「什么时候到点」，`recovery` 决定「忙到什么时候」，
/// `interrupt_resist` 决定被打断时掷骰防守方那一侧的底数
/// （见 [`InterruptEvent`](crate::timeline::InterruptEvent)）。
///
/// **它描述的是载荷，不是调度器**：具体值归各领域（`movement` 的移动 / 跳跃 / 翻滚、
/// `combat::skills` 的近战 / 火球 / 箭矢、`combat::defense` 的招架）。
/// 调度器只接住这个值，把它变成 [`ScheduledAction`](super::ScheduledAction) 的时间戳。
#[derive(Debug, Default, Clone, Copy, PartialEq)]
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
