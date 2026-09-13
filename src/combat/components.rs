//! 战斗基础组件：阵营与可碰撞标记。

use bevy::prelude::*;

/// 参战阵营。
///
/// 一个带值的组件（而不是 `Player` / `Enemy` 两个空标记）：
/// 目标过滤、友伤、AI 选敌都走同一条查询路径，未来加中立 / 第三方只加变体。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[reflect(Component)]
pub enum Faction {
    #[default]
    Player,
    Enemy,
}

/// 可被碰撞的实体标记（纯物理过滤：哪些实体参与碰撞检测）。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Collidable;
