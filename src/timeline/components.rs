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

/// 状态：**已声明**（刚落地、还没被 [`super::commit_bridge_system`] 升为 `Pending`）。
///
/// 它只活一帧：声明系统本帧挂上，提交桥本帧或下一帧摘掉。撤销看的就是它
/// （连同 `Pending`）——`Committed` 才表示"来不及撤了"。
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
/// 声明动作时移除，后摇结束时恢复（见 [`BusyRecovery`]）。
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

/// 行动实体上的**花费记录**：撤销时按它退还资源。
///
/// 只有"声明时就扣资源"的动作需要它（火球扣 2 精力）；移动不花精力，
/// 翻滚 / 招架是**执行时**才扣，撤销时本来就没扣。AI 的行动写 0 也无妨——
/// [`undo_system`](super::systems::undo_system) 只撤玩家的行动。
#[derive(Component, Reflect, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct ActionCost(pub u32);

/// **取消这个行动**要付多少精力（挂在行动实体上，由声明它的领域填）。
///
/// 没挂这个组件 = 免费（`0` 也一样）：**取消代价是行动自己的属性**，
/// 不是全局规则——"赶路时调整方向"应当免费，"大招打断"就要付代价。
#[derive(Component, Reflect, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct CancelCost(pub u32);

/// 这个行动**根本不给取消**（跳跃那种"起跳就谁都别想插队"）。
///
/// 和"进入结算 / 后摇"的区别：那是**时间窗口**（谁都撤不掉），
/// 这个是**动作属性**（前摇里也撤不掉）。
#[derive(Component, Reflect, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct Uncancellable;
