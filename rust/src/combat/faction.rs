//! 阵营：区分参战方（同一种组件、同一查询路径，支持以后多阵营）

use bevy::prelude::*;

/// 参战阵营。旧设计用 `Player` / `Enemy` 两个空标记，这里收敛为
/// 一个带值的组件：AI / 目标过滤 / 友伤规则都通过 `Faction` 查询。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Faction {
    #[default]
    Player,
    Enemy,
}
