pub mod aoe;
pub mod melee;
pub mod projectile;

use bevy::prelude::*;

// 单体目标
#[derive(Component)]
pub struct Target(pub Entity);

// 多体目标（用于 AOE、横扫等）
#[derive(Component)]
pub struct Targets(pub Vec<Entity>);
