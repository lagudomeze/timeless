//! 交互域的**画面**：悬停高亮方块 + 行动预演指示器（AOE 圆盘 / 近战扇形）。
//!
//! 这三样东西回答「**我指着哪一格、这一手会打到哪**」，是纯表现：只有网格、
//! 材质与 `Transform`，不翻译输入、不写游戏状态。它们与
//! [`super::pointer`] 的分工是单向的——那边产出 [`HoveredCell`]，这边读它：
//!
//! ```text
//! 光标 ─▶ pointer::hover_cell_system ─▶ HoveredCell ─┬─▶ visual::update_hover_highlight_system
//!                                                    └─▶ visual::update_preview_indicators_system
//! 菜单选中 ───────────────────────────────────────────┘
//! ```
//!
//! 它们留在交互域而不是搬去 `presentation`：那样会让表现层读本域的
//! `HoveredCell`（别人的 `Resource`），违反 import 规则。交互的**画面**与交互的
//! **翻译**是同一个功能的两半，放在一个域里最省事；域内部再按"读的 / 画的"分文件。
//!
//! 预演判据与点击派发**必须一致**：都是「攻击按距离二选一」（贴脸近战、否则火球）。
//! 改一边记得改另一边。

use bevy::prelude::*;
// 高亮方块不该投影：它只是一层指示，投出影子反而像实体
use bevy::light::NotShadowCaster;

use crate::combat::Faction;
use crate::combat::attack::{FIREBALL_RADIUS, MELEE_REACH, MenuSelection, SKILLS, SkillKind};
use crate::movement::CELL_SIZE;
use crate::movement::Cell;
use crate::world::{TerrainConfig, ground_position};

use super::components::{AoePreview, ConePreview, HoverHighlight, HoverTint, HoveredCell};

/// 高亮方块离地高度（世界单位）：贴着地但不和地面 z-fighting。
pub const HIGHLIGHT_LIFT: f32 = 0.03;
/// 高亮方块边长占一格的比例（留一点缝，看得出格与格的边界）。
pub const HIGHLIGHT_FILL: f32 = 0.92;

/// 空地：青。
pub const HOVER_GROUND: Color = Color::srgba(0.35, 0.95, 0.95, 0.35);
/// 自己脚下：蓝（= 玩家色）。
pub const HOVER_PLAYER: Color = Color::srgba(0.35, 0.55, 0.95, 0.35);
/// 敌人脚下：红（= 敌人色）。
pub const HOVER_ENEMY: Color = Color::srgba(0.95, 0.35, 0.35, 0.40);

/// 火球 AOE 预演色（红，半透明：这是"会伤到谁"的范围）。
pub const AOE_PREVIEW: Color = Color::srgba(0.95, 0.30, 0.25, 0.22);
/// 近战扇形预演色（琥珀，和技能栏选中色同族）。
pub const CONE_PREVIEW: Color = Color::srgba(0.95, 0.84, 0.42, 0.22);
/// 近战扇形的张角（弧度）：正面 120°。
pub const CONE_ARC: f32 = std::f32::consts::TAU / 3.0;

/// 开局生成唯一的高亮方块（之后只搬位置 / 改颜色）。
pub fn spawn_hover_highlight(mut commands: Commands) {
    commands.spawn_scene(bsn! {
        Name("HoverHighlight")
        HoverHighlight
        HoverTint(Color::NONE)
        Mesh3d(asset_value(Rectangle::new(
            CELL_SIZE * HIGHLIGHT_FILL,
            CELL_SIZE * HIGHLIGHT_FILL,
        )))
        MeshMaterial3d<StandardMaterial>(asset_value(StandardMaterial {
            base_color: Color::NONE,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            ..default()
        }))
        // 平铺在地面上（和单位阴影同一套做法）
        Transform {
            rotation: {Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)},
        }
        Visibility::Hidden
        // 高亮方块不该投影：它只是一层指示，投出影子反而像实体
        NotShadowCaster
    });
}

/// 开局生成两个预演指示器：火球 AOE 圆盘 + 近战扇形（都默认隐藏）。
///
/// 它们和悬停高亮是两回事：高亮回答"我指着哪一格"，预演回答"**这一手会打到哪**"。
pub fn spawn_preview_indicators(mut commands: Commands) {
    let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    commands.spawn_scene(bsn! {
        Name("AoePreview")
        AoePreview
        Mesh3d(asset_value(Circle::new(FIREBALL_RADIUS)))
        MeshMaterial3d<StandardMaterial>(asset_value(StandardMaterial {
            base_color: AOE_PREVIEW,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            ..default()
        }))
        Transform {
            rotation: {flat},
        }
        Visibility::Hidden
        NotShadowCaster
    });
    commands.spawn_scene(bsn! {
        Name("ConePreview")
        ConePreview
        // 扇形从局部 +X 轴张开；下面按"朝向悬停格"整体旋转
        Mesh3d(asset_value(CircularSector::new(MELEE_REACH, CONE_ARC)))
        MeshMaterial3d<StandardMaterial>(asset_value(StandardMaterial {
            base_color: CONE_PREVIEW,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            ..default()
        }))
        Transform {
            rotation: {flat},
        }
        Visibility::Hidden
        NotShadowCaster
    });
}

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
    occupants: Query<(&Cell, &Faction)>,
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
fn tint_for(cell: Cell, occupants: &Query<(&Cell, &Faction)>) -> Color {
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
    use bevy::scene::ScenePlugin;

    /// 场景工厂验收：BSN 建出来的实体必须**真的带上**标记组件。
    ///
    /// 这条守着一次真实的坑——`bsn!` 改成补丁式写法后，少写一行 `HoverHighlight`
    /// 或 `Visibility::Hidden` 不会编译报错，只会让高亮**永远显示 / 永远不显示**。
    /// 所以这里不看"长得对不对"，只钉死"标记在不在、默认藏没藏"。
    #[test]
    fn the_scene_factories_attach_their_markers() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins((AssetPlugin::default(), ScenePlugin))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>();
        app.add_systems(Startup, (spawn_hover_highlight, spawn_preview_indicators));
        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<(Entity, &Visibility), With<HoverHighlight>>();
        let highlights: Vec<(Entity, Visibility)> = query
            .iter(app.world())
            .map(|(entity, visibility)| (entity, *visibility))
            .collect();
        assert_eq!(highlights.len(), 1, "高亮方块应当只有一个");
        assert_eq!(
            highlights[0].1,
            Visibility::Hidden,
            "没悬停时就该藏着（少了 `Visibility::Hidden` 会整块糊在场上）"
        );

        // 两个预演指示器各一个，且都默认隐藏
        for (name, count) in [("AoePreview", 1), ("ConePreview", 1)] {
            let found = match name {
                "AoePreview" => app
                    .world_mut()
                    .query_filtered::<&Visibility, With<AoePreview>>()
                    .iter(app.world())
                    .count(),
                _ => app
                    .world_mut()
                    .query_filtered::<&Visibility, With<ConePreview>>()
                    .iter(app.world())
                    .count(),
            };
            assert_eq!(found, count, "{name} 应当由场景工厂建出来");
        }
        let mut hidden = app
            .world_mut()
            .query_filtered::<&Visibility, (With<AoePreview>, Without<HoverHighlight>)>();
        assert!(
            hidden.iter(app.world()).all(|v| *v == Visibility::Hidden),
            "预演指示器开局应当都是隐藏的"
        );
    }

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
            .spawn((cell, Faction::Enemy, Transform::default()));

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
            app.world_mut()
                .spawn((Cell::new(1, 0), Faction::Player, Transform::default()));
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
