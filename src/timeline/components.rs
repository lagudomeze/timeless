//! 行动实体与行动者的标记组件。
//!
//! 「这行动是什么」由载荷组件决定（`MoveAction` / `FireballAction` / `MeleeAction`…），
//! 调度数据在 [`ScheduledAction`](super::ScheduledAction)，行动者的三阶段在
//! [`DecisionSlot`](super::DecisionSlot)——本文件只放两者都要读的**规则标记**。

use bevy::prelude::*;

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
