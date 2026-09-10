//! 敌人行为组件。

use bevy::prelude::*;

/// 敌人决策参数：进入交战距离才行动，进入攻击距离才出手。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct EnemyBrain {
    /// 超过这个距离按兵不动（世界单位）
    pub engage_range: f32,
    /// 进入这个距离后可以射击
    pub attack_range: f32,
}

impl Default for EnemyBrain {
    fn default() -> Self {
        Self {
            engage_range: 12.0,
            attack_range: 5.0,
        }
    }
}

/// 攻击冷却（**按轮计**）。
///
/// We-Go 的节奏以「轮」为单位，冷却也用轮数表达：出手后要等若干轮才能再射，
/// 等待期间敌人会继续逼近。移动速度不在这里——那是单位自己的
/// [`MoveSpeed`](crate::movement::MoveSpeed)。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AttackCooldown {
    remaining_rounds: u32,
}

/// 出手后的冷却轮数。
const ATTACK_COOLDOWN_ROUNDS: u32 = 2;

impl AttackCooldown {
    /// 本轮能否出手。
    pub fn is_ready(&self) -> bool {
        self.remaining_rounds == 0
    }

    /// 出手后进入冷却。
    pub fn after_shot(&mut self) {
        self.remaining_rounds = ATTACK_COOLDOWN_ROUNDS;
    }

    /// 每轮结束走一格冷却。
    pub fn tick_round(&mut self) {
        self.remaining_rounds = self.remaining_rounds.saturating_sub(1);
    }
}
