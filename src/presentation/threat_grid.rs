//! 威胁格：把**敌人这一手威胁到的格**画在地面上（只读，纯表现）。
//!
//! ## 为什么值得画
//!
//! `Threatens { cells }` 是**决策层**的格子集合，早就被两处读着：
//! [`combat::reaction`](crate::combat::reaction) 用它决定要不要冻结世界，
//! [`ai`](crate::ai) 用它决定要不要躲——**唯独玩家看不见**。
//! 把它画出来，玩家看到的威胁图与 AI 用的是**同一份数据**，这正是
//! `docs/game-design.md`「AI 与玩家对称」那条设计的兑现。
//!
//! 它回答的是 [`docs/insight.md`](../../docs/insight.md) 第五节里最缺的那个问题：
//! 「**它打到哪一格**」——火球锁格，所以"往旁边走一步就安全"这件事，
//! 在此之前玩家只能靠猜。
//!
//! ## 分层
//!
//! 与 [`effects`](super::effects) 同级：**只读** `Threatens` / `TargetCell`，
//! 不改任何游戏状态、也不影响结算。`combat` 因此不需要知道"威胁要画出来"。
//!
//! ## 怎么画
//!
//! 一个**格位池**：开局建好 [`THREAT_TILE_POOL`] 个贴地薄片（都默认隐藏），
//! 每帧按当前的威胁格**只改位置与显隐**，不增删实体——与时间轴色块池、
//! 敌人面板行池同一个做法（帧内不产生实体分配）。
//!
//! 与鼠标的 [`HoveredCell`](crate::interaction::HoveredCell) 高亮是**两套独立的视觉**：
//! 悬停是"我指着哪一格"（青 / 蓝 / 红，实心），威胁是"哪一格会挨打"（橙红，更暗），
//! 各画各的、不抢同一个实体。威胁格贴得略低一点，两者叠在一起也不会 z-fighting。

use std::collections::BTreeSet;

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::combat::Faction;
use crate::combat::reaction::{ReactionSlot, TargetCell, Threatens};
use crate::movement::{CELL_SIZE, Cell};
use crate::timeline::ActionOf;
use crate::world::{TerrainConfig, ground_position};

/// 最多同时画多少格威胁。
///
/// 超出就按格坐标顺序丢掉靠后的——**不是随机丢**，所以同一局面每帧画的是同一批。
/// 真实场面里同时在飞的火球 / 挥出的横扫不过一两个，每一手威胁 1~3 格，
/// 8 个够用；真不够时该考虑的是"这一手要不要整条轨迹都画"（见本文件末尾）。
pub const THREAT_TILE_POOL: usize = 8;

/// 威胁格薄片的边长占一格的比例（留缝，看得出格与格的边界）。
pub const THREAT_TILE_FILL: f32 = 0.96;

/// 威胁格离地高度（世界单位）：**比悬停高亮略低**，两者叠一起时悬停在上面。
pub const THREAT_TILE_LIFT: f32 = 0.02;

/// 威胁色：橙红、比悬停高亮暗——它是"警告"，不是"我选中的东西"。
///
/// 与命中特效的敌人色同族（都是暖红），一眼看出"这是冲我来的"。
pub const THREAT_TINT: Color = Color::srgba(0.95, 0.42, 0.20, 0.30);

/// 池里的一个威胁格薄片（`index` 只是池内编号，不断言任何语义）。
///
/// 派生 `Reflect` 是为了**运行时能查它**（BRP 诊断锚点）：没注册的组件在
/// 远程协议里等于不存在，`world.query` 会静默返回空——看着像"没画出来"，
/// 其实只是查不到（这个坑在命中特效那里踩过一次）。
#[derive(Component, Reflect, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct ThreatTile {
    /// 池内编号
    pub index: usize,
}

/// 开局生成整池威胁格薄片（之后只搬位置 / 改显隐）。
pub fn spawn_threat_tiles(mut commands: Commands) {
    let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    for index in 0..THREAT_TILE_POOL {
        commands.spawn_scene(bsn! {
            Name("ThreatTile")
            ThreatTile { index: {index} }
            Mesh3d(asset_value(Rectangle::new(
                CELL_SIZE * THREAT_TILE_FILL,
                CELL_SIZE * THREAT_TILE_FILL,
            )))
            MeshMaterial3d<StandardMaterial>(asset_value(StandardMaterial {
                base_color: {THREAT_TINT},
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                double_sided: true,
                ..default()
            }))
            // 平铺在地面上（和单位阴影、悬停高亮同一套做法）
            Transform { rotation: {flat} }
            Visibility::Hidden
            // 只是一层指示，投出影子反而像实体
            NotShadowCaster
        });
    }
}

/// **威胁来源圈**：在「正在威胁你的那个单位」脚边套一圈。
///
/// 与威胁格（[`ThreatTile`]）是同一个问题的两半——**「它打哪」** 由格子回答，
/// **「谁在打我」** 由这一圈回答。两者都只在反应窗口开着时画。
///
/// 池化（[`THREAT_SOURCE_POOL`] 个）：一次只处理一个威胁，但池位留宽一点，
/// 免得将来"同帧多个来源"时又要改结构。
#[derive(Component, Reflect, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct ThreatSourceRing {
    /// 池内编号
    pub index: usize,
}

/// 威胁来源圈的池子大小。
///
/// 反应系统一次只挂一个 `ReactionSlot`（多威胁取**最先落地**的那个），
/// 所以 1 个就够用；留 2 个是为了将来"多条威胁同时可视化"时不必改结构。
pub const THREAT_SOURCE_POOL: usize = 2;

/// 威胁来源圈的环半径（世界单位）。
///
/// **必须大于阵营环的**（`FACTION_RING_OUTER`）：它套在阵营环外面当警示圈，
/// 两者叠在同一个单位脚下时不该互相盖住。有测试钉住这条不等式。
pub const THREAT_RING_RADIUS: f32 = 1.5;

/// 威胁来源圈的离地高度（世界单位）：比威胁格薄片略高，比阵营环略高。
pub const THREAT_RING_LIFT: f32 = 0.035;

/// 威胁来源圈的颜色：琥珀（与时间轴悬停指示圈、技能栏选中色同族）——
/// 它是"警告"，不是阵营信息（阵营由脚下的蓝/红环回答）。
pub const THREAT_RING_TINT: Color = Color::srgba(0.95, 0.72, 0.25, 0.55);

/// 开局生成整池威胁来源圈（之后只搬位置 / 改显隐）。
pub fn spawn_threat_source_rings(mut commands: Commands) {
    let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    for index in 0..THREAT_SOURCE_POOL {
        commands.spawn_scene(bsn! {
            Name("ThreatSourceRing")
            ThreatSourceRing { index: {index} }
            Mesh3d(asset_value(Annulus::new(
                THREAT_RING_RADIUS - THREAT_RING_WIDTH,
                THREAT_RING_RADIUS,
            )))
            MeshMaterial3d<StandardMaterial>(asset_value(StandardMaterial {
                base_color: {THREAT_RING_TINT},
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                double_sided: true,
                ..default()
            }))
            // 平铺在地面上（和单位阴影、威胁格同一套做法）
            Transform { rotation: {flat} }
            Visibility::Hidden
            // 只是一层指示，投出影子反而像实体
            NotShadowCaster
        });
    }
}

/// 威胁来源圈的环宽（世界单位）。
pub const THREAT_RING_WIDTH: f32 = 0.16;

/// 「正在威胁玩家的那个单位是谁」——**纯函数**，与反应系统的开窗判据同源。
///
/// 只看**开着的**窗口（`ReactionSlot` 存在且没表态）：窗口一关，圈就该消失，
/// 这与威胁格的判据一模一样（[`update_threat_grid_system`] 用同一份数据）。
///
/// 返回 `None` 的情形都该藏起来：没有窗口 / 已表态 / 来源不是行动实体
/// （飞行中的投射物没有"脚"，它的落点由威胁格回答）。
pub fn threat_source_of(
    slot: Option<&ReactionSlot>,
    threats: &Query<(&Threatens, Option<&ActionOf>)>,
) -> Option<Entity> {
    let slot = slot.filter(|slot| !slot.resolved)?;
    let (_, action_of) = threats.get(slot.threat).ok()?;
    Some(action_of?.actor())
}

/// 每帧把来源圈套到「正在威胁你的那个单位」脚下（没有就整池藏起来）。
///
/// **读的是那个单位此刻的 `Transform`**，不是它的格心：它在前摇里也可能被推着动
/// （翻滚、被击退），圈该跟着人走而不是钉在格子上。
pub fn update_threat_source_ring_system(
    terrain: Res<TerrainConfig>,
    slots: Query<&ReactionSlot>,
    threats: Query<(&Threatens, Option<&ActionOf>)>,
    // 两个查询都碰 `Transform`：用标记组件两两互斥（否则 Bevy 报 B0001）。
    // 单位带 `Faction`、池位带 `ThreatSourceRing`，两者不可能同时成立。
    positions: Query<&Transform, (With<Faction>, Without<ThreatSourceRing>)>,
    mut rings: Query<(&ThreatSourceRing, &mut Transform, &mut Visibility), Without<Faction>>,
) {
    // 一次只有一个窗口：取第一个（反应系统保证全局最多一个）
    let slot = slots.iter().next();
    let source = threat_source_of(slot, &threats)
        .and_then(|actor| positions.get(actor).ok())
        .map(|transform| transform.translation);

    for (ring, mut transform, mut visibility) in &mut rings {
        // 只有第 0 个池位画（池位留宽是为了将来扩展，不是现在就画多份）
        let Some(position) = source.filter(|_| ring.index == 0) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let ground = ground_position(&terrain, position.x, position.z);
        transform.translation = Vec3::new(position.x, ground.y + THREAT_RING_LIFT, position.z);
        *visibility = Visibility::Visible;
    }
}

///
/// 排序不是为了好看，是为了**每帧把同一批格分给同一批池位**——
/// 不排序的话 `Query` 的遍历顺序不保证稳定，薄片会在帧之间互相跳（看起来像闪）。
/// 去重则是因为两枚火球可以威胁到同一格，那格只该画一层。
/// **威胁格只有一处排序规则**：去重后按格坐标升序。
///
/// 排序不是为了好看，是为了**每帧把同一批格分给同一批池位**——
/// 不排序的话 `Query` 的遍历顺序不保证稳定，薄片会在帧之间互相跳（看起来像闪）。
/// 去重则是因为两枚火球可以威胁到同一格，那格只该画一层。
pub fn sorted_unique_cells(cells: impl IntoIterator<Item = Cell>) -> Vec<Cell> {
    let unique: BTreeSet<(i32, i32)> = cells.into_iter().map(|cell| (cell.x, cell.z)).collect();
    unique.into_iter().map(|(x, z)| Cell::new(x, z)).collect()
}

/// 每帧把威胁格画出来：按当前的威胁格摆池位，多出来的藏起来。
///
/// **威胁来源**（与 [`combat::reaction::detect_threat_system`](crate::combat::reaction::detect_threat_system)
/// 的判据一致）：
///
/// - **前摇中的敌方行动实体**看它自己的 `Threatens.cells`（横扫的扇形、火球的轨迹 + 落点）；
/// - **飞行中的敌方投射物**看它的 `TargetCell`（一枚正飞向某格的火球，同样是威胁）。
///
/// 「敌方」= 行动者阵营 ≠ 玩家阵营。玩家的自己人（将来可能的友方 NPC）不画。
pub fn update_threat_grid_system(
    terrain: Res<TerrainConfig>,
    threats: Query<(&Threatens, Option<&crate::timeline::ActionOf>)>,
    projectiles: Query<(&TargetCell, &Faction)>,
    actors: Query<&Faction>,
    mut tiles: Query<(&ThreatTile, &mut Transform, &mut Visibility)>,
) {
    let mut cells: Vec<Cell> = Vec::new();
    for (threat, action_of) in &threats {
        // 行动实体的阵营看它的**行动者**（`Threatens` 本身不带阵营）
        let hostile = action_of
            .and_then(|action_of| actors.get(action_of.actor()).ok())
            .is_none_or(|faction| *faction != Faction::Player);
        if hostile {
            cells.extend(threat.cells.iter().copied());
        }
    }
    for (target, faction) in &projectiles {
        if *faction != Faction::Player {
            cells.push(target.0);
        }
    }
    let cells = sorted_unique_cells(cells);

    for (tile, mut transform, mut visibility) in &mut tiles {
        let Some(cell) = cells.get(tile.index) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let center = cell.center();
        transform.translation =
            ground_position(&terrain, center.x, center.y) + Vec3::Y * THREAT_TILE_LIFT;
        *visibility = Visibility::Visible;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::reaction::ThreatKind;
    use crate::timeline::{ActionOf, ActionTiming, ScheduledAction};

    /// **来源圈与威胁格是同一个窗口的两半**：窗口开着 → 圈出行动者；关了就没了。
    #[test]
    fn the_source_ring_reads_the_same_window_as_the_threat_cells() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let enemy = app.world_mut().spawn(Faction::Enemy).id();
        let action = app
            .world_mut()
            .spawn((
                ActionOf(enemy),
                Threatens {
                    cells: vec![Cell::new(1, 1)],
                },
            ))
            .id();
        let make_slot = |resolved| ReactionSlot {
            threat: action,
            kind: ThreatKind::Incoming,
            suggestions: Vec::new(),
            resolved,
        };

        // 窗口开着且没表态 → 圈到**行动者**（不是行动实体）
        {
            let mut query = app.world_mut().query::<(&Threatens, Option<&ActionOf>)>();
            let threats = query.query(app.world());
            assert_eq!(
                threat_source_of(Some(&make_slot(false)), &threats),
                Some(enemy),
                "圈的是行动者（有脚的那个），不是行动实体"
            );
            assert_eq!(
                threat_source_of(Some(&make_slot(true)), &threats),
                None,
                "表过态就不该再圈——与威胁格「关窗即隐藏」同一条规矩"
            );
            assert_eq!(threat_source_of(None, &threats), None, "没有窗口就没有圈");
        }
    }

    /// 池子规模、默认隐藏、标记：少写一行不会编译报错，只表现为"永远显示"。
    #[test]
    fn the_source_pool_is_built_hidden_and_marked() {
        use bevy::scene::ScenePlugin;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins((AssetPlugin::default(), ScenePlugin))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>();
        app.add_systems(Startup, spawn_threat_source_rings);
        app.update();

        let mut query = app.world_mut().query::<(&ThreatSourceRing, &Visibility)>();
        let rings: Vec<(usize, Visibility)> = query
            .iter(app.world())
            .map(|(ring, visibility)| (ring.index, *visibility))
            .collect();
        assert_eq!(rings.len(), THREAT_SOURCE_POOL, "池子大小应当正好一池");
        assert!(
            rings
                .iter()
                .all(|(_, visibility)| *visibility == Visibility::Hidden),
            "开局整池都该藏着"
        );
    }

    /// **圈必须套在阵营环外面**：两者叠在同一个单位脚下，内环被盖住就没意义了。
    ///
    /// ⚠️ 比较的是常量，用 `black_box` 把值藏起来——否则 clippy 会说"断言的值恒定"
    /// （它是对的：常量本身不是测试，**改了常量这里会红**才是）。
    #[test]
    fn the_threat_ring_sits_outside_the_faction_ring() {
        let threat = std::hint::black_box(THREAT_RING_RADIUS);
        let faction = std::hint::black_box(crate::presentation::unit_sprite::FACTION_RING_OUTER);
        assert!(
            threat > faction,
            "威胁圈半径 {threat} 没有大过阵营环 {faction}——两层会糊在一起"
        );
    }

    /// 池位跟着**单位此刻的位置**走（不是它的格心）：前摇里被推着动时圈要跟着人。
    #[test]
    fn the_ring_follows_the_unit_not_its_cell() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TerrainConfig::default())
            .add_systems(Update, update_threat_source_ring_system);

        let enemy = app
            .world_mut()
            .spawn((Faction::Enemy, Transform::from_xyz(5.0, 0.0, 7.0)))
            .id();
        let action = app
            .world_mut()
            .spawn((
                ActionOf(enemy),
                Threatens {
                    cells: vec![Cell::new(0, 0)],
                },
                ActionTiming::default(),
                ScheduledAction::default(),
            ))
            .id();
        // 窗口挂在"玩家"身上（这里只需要有个带 `ReactionSlot` 的实体）
        let player = app
            .world_mut()
            .spawn(ReactionSlot {
                threat: action,
                kind: ThreatKind::Incoming,
                suggestions: Vec::new(),
                resolved: false,
            })
            .id();
        let ring = app
            .world_mut()
            .spawn((
                ThreatSourceRing { index: 0 },
                Transform::default(),
                Visibility::Hidden,
            ))
            .id();
        // 池位 1 也该保持隐藏（一次只画一个来源）
        let spare = app
            .world_mut()
            .spawn((
                ThreatSourceRing { index: 1 },
                Transform::default(),
                Visibility::Hidden,
            ))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<Visibility>(ring).unwrap(),
            &Visibility::Visible,
            "窗口开着 → 圈显示出来"
        );
        let translation = app.world().get::<Transform>(ring).unwrap().translation;
        assert!(
            (translation.x - 5.0).abs() < 1e-6 && (translation.z - 7.0).abs() < 1e-6,
            "圈的 XZ 应当跟着单位，实际 {translation:?}"
        );
        assert_eq!(
            app.world().get::<Visibility>(spare).unwrap(),
            &Visibility::Hidden,
            "一次只画一个来源"
        );

        // 表态之后：整池藏起来
        app.world_mut()
            .get_mut::<ReactionSlot>(player)
            .unwrap()
            .resolved = true;
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(ring).unwrap(),
            &Visibility::Hidden,
            "表过态就不该再圈"
        );
    }

    #[test]
    fn the_cells_are_deduped_and_sorted_for_a_stable_pool() {
        let cells = sorted_unique_cells([
            Cell::new(3, 1),
            Cell::new(1, 2),
            Cell::new(3, 1), // 同一格出现两次（两枚火球威胁同一格）
            Cell::new(1, 1),
        ]);
        assert_eq!(
            cells,
            vec![Cell::new(1, 1), Cell::new(1, 2), Cell::new(3, 1)],
            "去重 + 按格坐标升序：同一局面每帧必须得到同一个顺序（池位才不会跳）"
        );
    }

    /// 场景工厂验收：池里的薄片必须**真的带上标记、默认隐藏**。
    ///
    /// 少写一行 `ThreatTile` 或 `Visibility::Hidden` 不会编译报错，
    /// 只表现为"威胁格永远显示 / 永远不显示"——所以这里钉死标记与默认显隐。
    #[test]
    fn the_pool_is_built_hidden_and_marked() {
        use bevy::scene::ScenePlugin;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins((AssetPlugin::default(), ScenePlugin))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>();
        app.add_systems(Startup, spawn_threat_tiles);
        app.update();

        let mut query = app.world_mut().query::<(&ThreatTile, &Visibility)>();
        let tiles: Vec<(usize, Visibility)> = query
            .iter(app.world())
            .map(|(tile, visibility)| (tile.index, *visibility))
            .collect();
        assert_eq!(tiles.len(), THREAT_TILE_POOL, "池子大小应当正好一池");
        assert!(
            tiles
                .iter()
                .all(|(_, visibility)| *visibility == Visibility::Hidden),
            "开局整池都该藏着"
        );
    }

    fn grid_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TerrainConfig::default())
            .add_systems(Update, update_threat_grid_system);
        app
    }

    fn spawn_tiles(app: &mut App) -> Vec<Entity> {
        (0..THREAT_TILE_POOL)
            .map(|index| {
                app.world_mut()
                    .spawn((
                        ThreatTile { index },
                        Transform::default(),
                        Visibility::Hidden,
                    ))
                    .id()
            })
            .collect()
    }

    /// **敌方的威胁格被画出来**：一条敌方前摇中的横扫，它威胁的三格都亮起来。
    #[test]
    fn a_hostile_pending_action_paints_its_threatened_cells() {
        let mut app = grid_app();
        let tiles = spawn_tiles(&mut app);
        let enemy = app.world_mut().spawn(Faction::Enemy).id();
        let action = app
            .world_mut()
            .spawn((
                ActionOf(enemy),
                Threatens {
                    cells: vec![Cell::new(1, 0), Cell::new(1, 1), Cell::new(1, -1)],
                },
                ActionTiming::default(),
                ScheduledAction::default(),
            ))
            .id();
        let _ = action;

        app.update();

        let visible: Vec<&ThreatTile> = app
            .world_mut()
            .query_filtered::<&ThreatTile, With<Visibility>>()
            .iter(app.world())
            .filter(|tile| {
                *app.world()
                    .get::<Visibility>(tiles[tile.index])
                    .expect("池位存在")
                    == Visibility::Visible
            })
            .collect();
        assert_eq!(visible.len(), 3, "敌方横扫威胁的 3 格都该画出来");
    }

    /// **玩家自己的行动不画**：威胁图是"哪里会挨打"，不是"我打到哪"。
    #[test]
    fn the_players_own_action_paints_nothing() {
        let mut app = grid_app();
        let tiles = spawn_tiles(&mut app);
        let player = app.world_mut().spawn(Faction::Player).id();
        app.world_mut().spawn((
            ActionOf(player),
            Threatens {
                cells: vec![Cell::new(5, 5)],
            },
            ActionTiming::default(),
            ScheduledAction::default(),
        ));

        app.update();

        assert!(
            tiles
                .iter()
                .all(|tile| *app.world().get::<Visibility>(*tile).unwrap() == Visibility::Hidden),
            "玩家自己威胁的格不该亮起（那是「我打到哪」，不是「哪里会挨打」）"
        );
    }

    /// 没有威胁时整池藏着；威胁数超过池子大小时按顺序裁掉，不 panic。
    #[test]
    fn the_pool_hides_when_idle_and_clamps_when_overflowing() {
        let mut app = grid_app();
        let tiles = spawn_tiles(&mut app);
        app.update();
        assert!(
            tiles
                .iter()
                .all(|tile| *app.world().get::<Visibility>(*tile).unwrap() == Visibility::Hidden),
            "没有威胁时整池该藏着"
        );

        // 威胁格多到超过池子：只该画池子那么多，且不越界 panic
        let enemy = app.world_mut().spawn(Faction::Enemy).id();
        let many: Vec<Cell> = (0..THREAT_TILE_POOL as i32 + 5)
            .map(|x| Cell::new(x, 0))
            .collect();
        app.world_mut().spawn((
            ActionOf(enemy),
            Threatens { cells: many },
            ActionTiming::default(),
            ScheduledAction::default(),
        ));

        app.update();

        let shown = tiles
            .iter()
            .filter(|tile| *app.world().get::<Visibility>(**tile).unwrap() == Visibility::Visible)
            .count();
        assert_eq!(
            shown, THREAT_TILE_POOL,
            "超出的威胁格按顺序裁掉，不是随机丢"
        );
    }
}
