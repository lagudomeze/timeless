//! # 战斗基础组件
//!
//! 全部是独立小组件，每种只承载一个职责；用 `bsn!` 生成实体时组件需
//! `Default + Clone`（自动得到 `FromTemplate`），含 `Entity` 字段的
//! 显式派生 `FromTemplate`。

use bevy::ecs::template::FromTemplate;
use bevy::prelude::*;

/// 速度：任何带 `Transform` + [`Velocity`] 的实体都能被移动系统驱动
/// （玩家 / 敌人 / 射弹通用，减速、击退只需改本值）。
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Velocity(pub Vec3);

/// 发射者：用于过滤自己（生成攻击实体的阵营归属）
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, FromTemplate)]
pub struct Owner(pub Entity);

/// 射弹逻辑状态（自包含：穿透次数、当前计数、结束标志）
#[derive(Component, Debug, Clone)]
pub struct Projectile {
    /// 最大穿透次数（1 = 普通，只会命中一个目标；0 = 无限）
    pub max_hits: usize,
    /// 当前已命中次数（唯一计数源，避免 `hit_targets` 冗余集合）
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

/// 可被碰撞的实体标记（纯物理过滤：哪些实体参与碰撞检测）
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Collidable;

/// 命中半径（射弹与目标各有一个；距离 ≤ 两者半径之和即接触）
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct HitRadius(pub f32);

impl Default for HitRadius {
    fn default() -> Self {
        Self(0.5)
    }
}

/// 碰撞候选标记（临时）：由 `targeting` 每帧挂到射弹上、`lifecycle` 消费移除。
/// 伤害系统只认这个标记，不关心目标是碰撞来的还是 AOE 扫到的。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, FromTemplate)]
pub struct CollisionTarget(pub Entity);

/// 物理伤害值（具体攻击挂载）
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct PhysicalDamage(pub f32);

/// 护甲（目标挂载，物理伤害先扣护甲）
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Armor(pub f32);

/// 近战命中形状：攻击实体以自身为圆心、朝 `Transform` 正前方扫扇形，
/// 只命中一次。数值进组件，目标获取与伤害完全解耦。
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct MeleeShape {
    pub range: f32,
    /// 扇形半角（弧度）
    pub half_arc: f32,
}

impl Default for MeleeShape {
    fn default() -> Self {
        Self {
            range: 2.5,
            half_arc: 60.0_f32.to_radians(),
        }
    }
}

/// 一次性命中开关：近战等攻击实体用，命中一次后不再重复结算。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HitOnce {
    pub spent: bool,
}

/// 攻击实体存活时长（近战横扫等一次性攻击到期自动销毁）
#[derive(Component, Debug, Clone)]
pub struct Lifetime(pub Timer);

impl Default for Lifetime {
    fn default() -> Self {
        Self(Timer::from_seconds(0.18, TimerMode::Once))
    }
}
