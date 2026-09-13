//! 交互域系统：光标 → 世界格 → 高亮实体。
//!
//! 两个系统分工明确：`hover_cell_system` 只回答"现在指着哪一格"（写资源），
//! `update_hover_highlight_system` 只负责把那个答案画出来。中间那层资源是
//! **纯数据**，所以"高亮没出现"时可以先读它，一眼分辨是拾取问题还是画面问题。

use bevy::prelude::*;
// 高亮方块不该投影：它只是一层指示，投出影子反而像实体
use bevy::light::NotShadowCaster;
use bevy::window::PrimaryWindow;

use crate::combat::Faction;
use crate::combat::skills::{
    FIREBALL_RADIUS, MELEE_REACH, MenuSelection, SKILLS, SkillKind, UseSelectedSkill,
};
use crate::movement::Cell;
use crate::movement::MoveToCommand;
use crate::presentation::MainCamera;
use crate::presentation::hud::PreviewReadout;
use crate::timeline::{CELL_SIZE, UndoCommand};
use crate::world::{TerrainConfig, ground_position};

use super::components::{AoePreview, ConePreview, HoverHighlight, HoverTint, HoveredCell};
use super::events::PointerCommand;
use super::raycast::{cursor_ray, pick_cell};

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
pub fn spawn_hover_highlight(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Name::new("HoverHighlight"),
        HoverHighlight,
        HoverTint(Color::NONE),
        Mesh3d(meshes.add(Rectangle::new(
            CELL_SIZE * HIGHLIGHT_FILL,
            CELL_SIZE * HIGHLIGHT_FILL,
        ))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::NONE,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            ..default()
        })),
        // 平铺在地面上（和单位阴影同一套做法）
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        Visibility::Hidden,
        NotShadowCaster,
    ));
}

/// 每帧：光标 → 世界格，只在变化时写资源。
///
/// **不读任何 `Time`**：玩家等输入时虚拟时间是冻结的，但悬停必须照常响应。
pub fn hover_cell_system(
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    terrain: Res<TerrainConfig>,
    mut hovered: ResMut<HoveredCell>,
) {
    let next = windows
        .single()
        .ok()
        .and_then(|window| window.cursor_position())
        .and_then(|cursor| {
            let (camera, transform) = cameras.iter().next()?;
            cursor_ray(camera, transform, cursor)
        })
        .and_then(|ray| pick_cell(ray, &terrain));

    if hovered.0 != next {
        hovered.0 = next;
    }
}

/// 开局生成两个预演指示器：火球 AOE 圆盘 + 近战扇形（都默认隐藏）。
///
/// 它们和悬停高亮是两回事：高亮回答"我指着哪一格"，预演回答"**这一手会打到哪**"。
pub fn spawn_preview_indicators(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut material = |color: Color| {
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: color,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            ..default()
        }))
    };

    let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    commands.spawn((
        Name::new("AoePreview"),
        AoePreview,
        Mesh3d(meshes.add(Circle::new(FIREBALL_RADIUS))),
        material(AOE_PREVIEW),
        Transform::from_rotation(flat),
        Visibility::Hidden,
        NotShadowCaster,
    ));
    commands.spawn((
        Name::new("ConePreview"),
        ConePreview,
        // 扇形从局部 +X 轴张开；下面按"朝向悬停格"整体旋转
        Mesh3d(meshes.add(CircularSector::new(MELEE_REACH, CONE_ARC))),
        material(CONE_PREVIEW),
        Transform::from_rotation(flat),
        Visibility::Hidden,
        NotShadowCaster,
    ));
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

/// 预演读数：把「这一手会变成什么 · 多远 · 预计多少伤害」写到 HUD 的提示条上。
///
/// 只在**文案真的变了**时发消息——每帧发会把 HUD 的淡出节奏打乱，也白白唤醒 UI。
#[allow(clippy::too_many_arguments)]
pub fn update_preview_readout_system(
    hovered: Res<HoveredCell>,
    selection: Res<MenuSelection>,
    terrain: Res<TerrainConfig>,
    units: Query<(&Transform, &Faction)>,
    mut readouts: MessageWriter<PreviewReadout>,
    mut last: Local<Option<String>>,
) {
    let kind = SKILLS
        .get(selection.index())
        .map(|def| def.kind)
        .unwrap_or(SkillKind::Attack);
    let player = units
        .iter()
        .find(|(_, faction)| **faction == Faction::Player)
        .map(|(transform, _)| transform.translation);

    let text = match (player, hovered.0) {
        (Some(player), Some(cell)) => {
            let center = cell.center();
            let target = ground_position(&terrain, center.x, center.y);
            let distance = player.distance(target);
            // 「攻击」按距离派发：读数直接告诉玩家这一下会变成近战还是火球
            let effective = match kind {
                SkillKind::Attack if distance <= MELEE_REACH => SkillKind::Melee,
                SkillKind::Attack => SkillKind::Fireball,
                other => other,
            };
            let power = SKILLS
                .iter()
                .find(|def| def.kind == effective)
                .map(|def| def.power)
                .unwrap_or_default();
            Some(format!(
                "{} · cell ({},{}) · dist {distance:.1} · dmg {power:.0}",
                effective.label().to_uppercase(),
                cell.x,
                cell.z
            ))
        }
        _ => None,
    };

    if text != *last {
        *last = text.clone();
        readouts.write(PreviewReadout(text));
    }
}

/// 点击 → 各领域的消息。
///
/// 规则（就是鼠标左 / 右键的语义）：
///
/// | 操作 | 情形 | 结果 |
/// | :--- | :--- | :--- |
/// | 左键 | 点在单位上（敌我皆可） | 用**当前选中的技能**打这一格 |
/// | 左键 | 点在空地上 | 走到这一格（`MoveToCommand`，可跨多格） |
/// | 右键 | —— | 撤销最近一条未结算的玩家行动（`UndoCommand`） |
///
/// 本系统只**写消息**，落地归各自的领域——和 `input` 同一条约定。
#[allow(clippy::too_many_arguments)]
pub fn pointer_command_system(
    mut clicks: MessageReader<PointerCommand>,
    hovered: Res<HoveredCell>,
    occupants: Query<(&Cell, &Faction)>,
    mut moves: MessageWriter<MoveToCommand>,
    mut skills: MessageWriter<UseSelectedSkill>,
    mut undos: MessageWriter<UndoCommand>,
) {
    for click in clicks.read() {
        match click {
            PointerCommand::Secondary => {
                undos.write(UndoCommand);
            }
            PointerCommand::Primary => {
                let Some(cell) = hovered.0 else {
                    continue; // 没指到地面（指到天空 / 光标不在窗口里）
                };
                if occupants.iter().any(|(occupied, _)| *occupied == cell) {
                    skills.write(UseSelectedSkill {
                        target_cell: Some(cell),
                    });
                } else {
                    moves.write(MoveToCommand { cell });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 探针：把本帧写出的消息数成计数，便于断言"点了哪一枪"。
    #[derive(Resource, Default)]
    struct Probes {
        moves: usize,
        skills: usize,
        undos: usize,
    }

    #[allow(clippy::too_many_arguments)]
    fn probe_system(
        mut probes: ResMut<Probes>,
        mut moves: MessageReader<MoveToCommand>,
        mut skills: MessageReader<UseSelectedSkill>,
        mut undos: MessageReader<UndoCommand>,
    ) {
        probes.moves += moves.read().count();
        probes.skills += skills.read().count();
        probes.undos += undos.read().count();
    }

    fn click_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<HoveredCell>()
            .init_resource::<Probes>()
            .add_message::<PointerCommand>()
            .add_message::<MoveToCommand>()
            .add_message::<UseSelectedSkill>()
            .add_message::<UndoCommand>()
            .add_systems(Update, (pointer_command_system, probe_system).chain());
        app
    }

    /// 左键的两种去向：空地板 → 移动；有单位 → 用当前技能（没有"确认"这一步）。
    #[test]
    fn primary_click_picks_the_right_command() {
        let cell = Cell::new(2, 2);

        // ① 空地板
        let mut app = click_app();
        app.insert_resource(HoveredCell(Some(cell)));
        app.world_mut().write_message(PointerCommand::Primary);
        app.update();
        let probes = app.world().resource::<Probes>();
        assert_eq!((probes.moves, probes.skills), (1, 0), "点地板 = 走过去");

        // ② 那格上站着单位
        let mut app = click_app();
        app.insert_resource(HoveredCell(Some(cell)));
        app.world_mut()
            .spawn((cell, Faction::Enemy, Transform::default()));
        app.world_mut().write_message(PointerCommand::Primary);
        app.update();
        let probes = app.world().resource::<Probes>();
        assert_eq!((probes.moves, probes.skills), (0, 1), "点单位 = 用当前技能");
    }

    /// 右键 = 撤销。
    #[test]
    fn secondary_click_asks_for_undo() {
        let mut app = click_app();
        app.world_mut().write_message(PointerCommand::Secondary);
        app.update();

        let probes = app.world().resource::<Probes>();
        assert_eq!((probes.undos, probes.moves), (1, 0), "右键只请求撤销");
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
