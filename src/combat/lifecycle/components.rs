//! 攻击生命周期组件。

use bevy::prelude::*;

/// 射弹逻辑状态（自包含：穿透次数、当前计数、结束标志）。
#[derive(Component, Debug, Clone)]
pub struct Projectile {
    /// 最大穿透次数（1 = 普通，只命中一个目标；0 = 无限）
    pub max_hits: usize,
    /// 已命中次数（唯一计数源）
    pub current_hits: usize,
    /// 是否已结束（速度归零，等待清理）
    pub finished: bool,
}

impl Default for Projectile {
    fn default() -> Self {
        Self {
            max_hits: 1,
            current_hits: 0,
            finished: false,
        }
    }
}

/// 一次性命中开关：近战等攻击实体命中一次后不再重复结算。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HitOnce {
    pub spent: bool,
}

/// 攻击实体存活时长（近战横扫等一次性攻击到期自动销毁）。
#[derive(Component, Debug, Clone)]
pub struct Lifetime(pub Timer);

impl Default for Lifetime {
    fn default() -> Self {
        Self(Timer::from_seconds(0.18, TimerMode::Once))
    }
}
