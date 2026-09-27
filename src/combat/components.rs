//! 战斗基础组件：阵营与可碰撞标记。

use bevy::prelude::*;

/// 参战阵营。
///
/// 一个带值的组件（而不是 `Player` / `Enemy` 两个空标记）：
/// 目标过滤、友伤、AI 选敌都走同一条查询路径，未来加中立 / 第三方只加变体。
///
/// ⚠️ **`Faction` ≠「单位」**：攻击实体（箭矢 / 横扫 / 火球，见
/// [`crate::combat::attack::scene`]）**也带 `Faction`**——命中过滤要靠它"不打自己人"。
/// 所以**"场上都有谁"这类查询不能只按 `Faction` 收**，要配下面的 [`Collidable`]。
/// 2026-09-27 一天之内因为漏了这一条，抓到三处"把箭当成单位"的 bug：
/// 时间轴给它一条车道并让它站进候场区、镜头追着它走、它还会长出贴地阴影。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[reflect(Component)]
pub enum Faction {
    #[default]
    Player,
    Enemy,
}

/// **「这是一个单位」的标记**：可被碰撞的实体（纯物理过滤：哪些实体参与碰撞检测）。
///
/// ⚠️ **它同时是"单位"的唯一判据**，因为它的挂载点**只有一处**：
/// [`crate::spawn::unit::unit_scene`]（组装单位时挂上）。攻击实体没有它，
/// 所以 `With<Collidable>` 精确表示"一个站在场上的单位"。
///
/// **凡是要"枚举场上的单位"的查询，都该配它**——不要只写 `With<Faction>`，
/// 那会把箭矢 / 火球一起收进来（见 [`Faction`] 的说明）。
/// 现成范例：`combat::targeting::detection` / `melee`、
/// `presentation::hud::timeline` 的 roster、`presentation::unit_sprite`、
/// `interaction::pointer` 的拾取。
///
/// 反面教材（同样要判据、但用的是更贴切的组件）：
/// - "谁是玩家"用 [`crate::timeline::InputDriven`]（`presentation::camera`、
///   `presentation::hud::skills`）——`Faction::Player` 同样会被玩家的箭满足；
/// - "格子上有没有人"可以用 `Cell`。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Collidable;
