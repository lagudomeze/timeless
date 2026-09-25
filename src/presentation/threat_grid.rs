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
use crate::combat::reaction::{TargetCell, Threatens};
use crate::movement::{CELL_SIZE, Cell};
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
    use crate::timeline::{ActionOf, ActionTiming, ScheduledAction};

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
