//! 交互域的组件与资源：**当前悬停的格** + 悬停高亮实体。

use bevy::prelude::*;

use crate::movement::Cell;

/// 本帧光标悬停的格（`None` = 没指到地面 / 没有光标）。
///
/// 存成**资源**而不是组件：它是"这一帧的输入状态"，只可能有一份；
/// 注册进反射后 BRP 能直接读它，排查"高亮没出现"时先看它是不是 `None`。
#[derive(Resource, Reflect, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[reflect(Resource)]
pub struct HoveredCell(pub Option<Cell>);

/// 悬停高亮实体（开局生成一个，之后只搬位置 / 改颜色，不增删实体）。
#[derive(Component, Reflect, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct HoverHighlight;

/// 高亮的当前颜色（**进组件**而不是只藏在材质里：BRP 能直接读出来排查）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq)]
#[reflect(Component)]
pub struct HoverTint(pub Color);

/// 火球预演：悬停格上的 AOE 圆盘（半径 = `FIREBALL_RADIUS`）。
#[derive(Component, Reflect, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct AoePreview;

/// 近战预演：玩家脚边的扇形（朝向悬停格）。
#[derive(Component, Reflect, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct ConePreview;
