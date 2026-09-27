//! 交互域的**每帧画面**：悬停高亮的颜色 / 位置，以及两个预演指示器的显隐。
//!
//! **建实体不在这里**——三个指示器的网格与材质在 [`super::scene`]（那是本域唯一
//! 会用 `asset_value(...)` 的地方）。本文件只做"读状态 → 改 `Transform` / 颜色 / 显隐"。
//!
//! 数据流是**单向**的：那边产出 [`HoveredCell`]，这里读它。
//!
//! ```text
//! 光标 ─▶ pointer::hover_cell_system ─▶ HoveredCell ─┬─▶ update_hover_highlight_system
//!                                                    └─▶ update_preview_indicators_system
//! 菜单选中 ───────────────────────────────────────────┘
//! ```
//!
//! 它们留在交互域而不是搬去 `presentation`：那样会让表现层读本域的
//! `HoveredCell`（别人的 `Resource`），违反 import 规则。交互的**画面**与交互的
//! **翻译**是同一个功能的两半，放在一个域里最省事；域内部再按"读的 / 画的"分文件：
//! **建实体归 [`super::scene`]，每帧刷状态归这里**。
//!
//! 预演判据与点击派发**必须一致**：都是「攻击按距离二选一」（贴脸近战、否则火球）。
//! 改一边记得改另一边。

use bevy::prelude::*;

use crate::combat::attack::{MELEE_REACH, MenuSelection, SKILLS, SkillKind};
use crate::combat::{Collidable, Faction};
use crate::movement::Cell;
use crate::world::{TerrainConfig, ground_position};

use super::components::{AoePreview, ConePreview, HoverHighlight, HoverTint, HoveredCell};
use super::scene::{HIGHLIGHT_LIFT, HOVER_ENEMY, HOVER_GROUND, HOVER_PLAYER};

/// 按**当前选中的技能**决定显示哪个预演：火球给 AOE 圆盘、近战给扇形。
///
/// 「攻击」是距离派发（贴脸近战、否则火球），所以它按悬停距离二选一，和
/// `use_selected_skill_system` 的判据保持一致。
/// 预演系统的三个查询：都碰 `Transform`，用标记组件两两互斥（B0001）。
type PreviewUnitQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Cell, &'static Transform, &'static Faction),
    (
        With<Collidable>,
        Without<AoePreview>,
        Without<ConePreview>,
        Without<HoverHighlight>,
    ),
>;

/// 见 [`PreviewUnitQuery`]。
type PreviewAoeQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Transform, &'static mut Visibility),
    (
        With<AoePreview>,
        Without<ConePreview>,
        Without<HoverHighlight>,
    ),
>;

/// 见 [`PreviewUnitQuery`]。
type PreviewConeQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Transform, &'static mut Visibility),
    (
        With<ConePreview>,
        Without<AoePreview>,
        Without<HoverHighlight>,
    ),
>;

pub fn update_preview_indicators_system(
    hovered: Res<HoveredCell>,
    selection: Res<MenuSelection>,
    terrain: Res<TerrainConfig>,
    units: PreviewUnitQuery<'_, '_>,
    mut aoe: PreviewAoeQuery<'_, '_>,
    mut cone: PreviewConeQuery<'_, '_>,
) {
    let kind = SKILLS
        .get(selection.index())
        .map(|def| def.kind)
        .unwrap_or(SkillKind::Attack);
    let actor = units
        .iter()
        .find(|(_, _, faction)| **faction == Faction::Player)
        .map(|(_, transform, _)| transform.translation);

    // 悬停格与玩家的关系：决定「攻击」这一手到底是近战还是火球
    let target = hovered.0.map(|cell| {
        let center = cell.center();
        ground_position(&terrain, center.x, center.y)
    });
    let distance = match (actor, target) {
        (Some(actor), Some(target)) => Some(actor.distance(target)),
        _ => None,
    };

    let show_cone = matches!(kind, SkillKind::Melee)
        || (kind == SkillKind::Attack && distance.is_some_and(|d| d <= MELEE_REACH));
    let show_aoe = matches!(kind, SkillKind::Fireball)
        || (kind == SkillKind::Attack && distance.is_some_and(|d| d > MELEE_REACH));

    for (mut transform, mut visibility) in &mut aoe {
        match (show_aoe, target) {
            (true, Some(target)) => {
                transform.translation = target + Vec3::Y * HIGHLIGHT_LIFT;
                *visibility = Visibility::Visible;
            }
            _ => *visibility = Visibility::Hidden,
        }
    }

    for (mut transform, mut visibility) in &mut cone {
        match (show_cone, actor, target) {
            (true, Some(actor), Some(target)) => {
                // 扇形贴地摆在玩家脚边，并朝悬停格转过去
                let to_target = (target - actor).with_y(0.0).normalize_or_zero();
                let yaw = if to_target == Vec3::ZERO {
                    0.0
                } else {
                    to_target.x.atan2(-to_target.z)
                };
                transform.translation = actor + Vec3::Y * HIGHLIGHT_LIFT;
                transform.rotation = Quat::from_rotation_y(yaw)
                    * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
                *visibility = Visibility::Visible;
            }
            _ => *visibility = Visibility::Hidden,
        }
    }
}

/// 把高亮方块搬到悬停格的地表上；没悬停就藏起来。
///
/// 颜色只在真的变了才写材质（材质在 `Assets` 里，改一次要过一遍资源）。
pub fn update_hover_highlight_system(
    hovered: Res<HoveredCell>,
    terrain: Res<TerrainConfig>,
    occupants: Query<(&Cell, &Faction), With<Collidable>>,
    mut highlights: Query<
        (
            &mut Transform,
            &mut Visibility,
            &mut HoverTint,
            &MeshMaterial3d<StandardMaterial>,
        ),
        With<HoverHighlight>,
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (mut transform, mut visibility, mut tint, material) in &mut highlights {
        let Some(cell) = hovered.0 else {
            *visibility = Visibility::Hidden;
            continue;
        };

        let center = cell.center();
        transform.translation =
            ground_position(&terrain, center.x, center.y) + Vec3::Y * HIGHLIGHT_LIFT;
        *visibility = Visibility::Visible;

        let next = tint_for(cell, &occupants);
        if next != tint.0 {
            tint.0 = next;
            // 0.19 的 `Assets::get_mut` 返回 `AssetMut`（智能指针），绑定要 `mut` 才能解引用改
            if let Some(mut tinted) = materials.get_mut(&material.0) {
                tinted.base_color = next;
            }
        }
    }
}

/// 悬停色：空地上是青的；踩到单位就按那个单位的阵营上色。
fn tint_for(cell: Cell, occupants: &Query<(&Cell, &Faction), With<Collidable>>) -> Color {
    occupants
        .iter()
        .find(|(occupied, _)| **occupied == cell)
        .map(|(_, faction)| match faction {
            Faction::Player => HOVER_PLAYER,
            Faction::Enemy => HOVER_ENEMY,
        })
        .unwrap_or(HOVER_GROUND)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn highlight_app(hovered: Option<Cell>) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<StandardMaterial>>()
            .insert_resource(TerrainConfig::default())
            .insert_resource(HoveredCell(hovered))
            .add_systems(Update, update_hover_highlight_system);
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color: Color::NONE,
                ..default()
            });
        let entity = app
            .world_mut()
            .spawn((
                HoverHighlight,
                HoverTint(Color::NONE),
                Transform::default(),
                Visibility::Hidden,
                MeshMaterial3d(material),
            ))
            .id();
        (app, entity)
    }

    /// 悬停时方块出现在那一格的地表上；移开就藏起来。
    #[test]
    fn the_highlight_follows_the_hovered_cell() {
        let cell = Cell::new(2, 1);
        let (mut app, entity) = highlight_app(Some(cell));

        app.update();

        let terrain = *app.world().resource::<TerrainConfig>();
        let center = cell.center();
        let expected = ground_position(&terrain, center.x, center.y).y + HIGHLIGHT_LIFT;
        let transform = *app.world().get::<Transform>(entity).unwrap();
        assert_eq!(
            app.world().get::<Visibility>(entity).unwrap(),
            &Visibility::Visible,
            "悬停时高亮应当可见"
        );
        assert_eq!(transform.translation.y, expected, "高亮要贴着那一格的地表");
        assert_eq!(
            (transform.translation.x, transform.translation.z),
            (center.x, center.y),
            "高亮要落在悬停格的中心"
        );
    }

    /// 没有悬停就没有高亮（`HoveredCell` 为 `None`）。
    #[test]
    fn no_hover_means_no_highlight() {
        let (mut app, entity) = highlight_app(None);

        app.update();

        assert_eq!(
            app.world().get::<Visibility>(entity).unwrap(),
            &Visibility::Hidden
        );
    }

    /// 悬停到单位身上时，颜色跟着阵营走。
    #[test]
    fn hovering_a_unit_tints_by_its_faction() {
        let cell = Cell::new(0, 2);
        let (mut app, entity) = highlight_app(Some(cell));
        app.world_mut()
            .spawn((cell, Faction::Enemy, Collidable, Transform::default()));

        app.update();

        assert_eq!(app.world().get::<HoverTint>(entity).unwrap().0, HOVER_ENEMY);
    }

    /// 预演指示器跟着**当前选中的技能**换：火球 → AOE 圆盘，近战 → 扇形，翻滚 → 都不显示。
    #[test]
    fn the_preview_switches_with_the_selected_skill() {
        fn preview_app(skill_index: usize, hovered: Option<Cell>) -> (App, Entity, Entity) {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .insert_resource(TerrainConfig::default())
                .insert_resource(HoveredCell(hovered))
                .init_resource::<MenuSelection>()
                .add_systems(Update, update_preview_indicators_system);
            app.world_mut()
                .resource_mut::<MenuSelection>()
                .select(skill_index);
            // 玩家站在 (1,0)，悬停格离它够远
            app.world_mut().spawn((
                Cell::new(1, 0),
                Faction::Player,
                Collidable,
                Transform::default(),
            ));
            let aoe = app
                .world_mut()
                .spawn((AoePreview, Transform::default(), Visibility::Hidden))
                .id();
            let cone = app
                .world_mut()
                .spawn((ConePreview, Transform::default(), Visibility::Hidden))
                .id();
            (app, aoe, cone)
        }
        let visible = |app: &App, entity: Entity| {
            *app.world().get::<Visibility>(entity).unwrap() == Visibility::Visible
        };
        let hovered = Cell::new(3, 0);

        // 火球（下标 2）
        let (mut app, aoe, cone) = preview_app(2, Some(hovered));
        app.update();
        assert!(visible(&app, aoe), "选火球应当显示 AOE 圆盘");
        assert!(!visible(&app, cone), "选火球不该显示近战扇形");
        let terrain = *app.world().resource::<TerrainConfig>();
        let center = hovered.center();
        let ground = ground_position(&terrain, center.x, center.y);
        assert_eq!(
            app.world().get::<Transform>(aoe).unwrap().translation,
            ground + Vec3::Y * HIGHLIGHT_LIFT,
            "AOE 圆盘要落在悬停格的地表上"
        );

        // 近战（下标 1）
        let (mut app, aoe, cone) = preview_app(1, Some(hovered));
        app.update();
        assert!(!visible(&app, aoe), "选近战不该显示 AOE 圆盘");
        assert!(visible(&app, cone), "选近战应当显示扇形");

        // 翻滚（下标 3）：两个都不显示
        let (mut app, aoe, cone) = preview_app(3, Some(hovered));
        app.update();
        assert!(!visible(&app, aoe) && !visible(&app, cone));
    }
}
