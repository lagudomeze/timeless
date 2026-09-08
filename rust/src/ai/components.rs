//! 敌人行为组件
use bevy::prelude::*;

/// 敌人决策参数：接近距离内持续追踪，进入攻击距离才出手。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct EnemyBrain {
    /// 进入此距离前只追踪（世界单位）
    pub engage_range: f32,
    /// 进入此距离后停住并发起攻击
    pub attack_range: f32,
    /// 追踪速度（格/秒）
    pub move_speed: f32,
}

impl Default for EnemyBrain {
    fn default() -> Self {
        Self {
            engage_range: 12.0,
            attack_range: 5.0,
            move_speed: 2.0,
        }
    }
}

/// 两次攻击之间的间隔（Timer 每 tick 结束后即可再次出手）。
#[derive(Component, Debug, Clone)]
pub struct AttackCooldown(pub Timer);

impl Default for AttackCooldown {
    fn default() -> Self {
        Self(Timer::from_seconds(0.8, TimerMode::Repeating))
    }
}
