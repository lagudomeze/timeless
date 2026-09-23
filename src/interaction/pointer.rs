//! 指针侧：**光标 → 格**（[`hover_cell_system`]）与**点击 → 消息**
//! （[`pointer_command_system`]），外加给 HUD 的预演读数
//! （[`update_preview_readout_system`]）。
//!
//! 这三个系统是交互域的"翻译"那一半：读世界（地形高度、单位所在格）与相机，
//! 写自己的资源与各领域的消息，**不产生任何实体、不碰网格与材质**。
//! 画出来的那一半在 [`super::visual`]。

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::combat::Faction;
use crate::combat::attack::{MELEE_REACH, MenuSelection, SKILLS, SkillKind, UseSelectedSkill};
use crate::combat::reaction::ReactionAnswer;
use crate::movement::Cell;
use crate::movement::MoveToCommand;
use crate::presentation::MainCamera;
use crate::presentation::hud::PreviewReadout;
use crate::timeline::{PlayerTakeover, UndoCommand};
use crate::world::{TerrainConfig, ground_position};

use super::components::HoveredCell;
use super::events::PointerCommand;
use super::raycast::{cursor_ray, pick_cell};

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
            let def = SKILLS.iter().find(|def| def.kind == effective);
            let power = def.map(|def| def.power).unwrap_or_default();
            // 速度帧（`AttackFrame`）：无回合模型里"到点"由 `execute_at` 决定，
            // 帧退居**信息层**——它是玩家判断「谁先动」的读数（见 `docs/game-design.md`
            // 的「洞察力」）。这里把它摆到预演读数上，字段因此有了真正的消费者。
            let frame = def.map(|def| def.frame).unwrap_or_default();
            let frame_text = if frame > 0 {
                format!(" · frame {frame}")
            } else {
                String::new() // 翻滚这类不产生攻击实体的动作没有帧
            };
            Some(format!(
                "{} · cell ({},{}) · dist {distance:.1} · dmg {power}{frame_text}",
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
/// 左键同时写一条 [`PlayerTakeover`]：它是"玩家这一帧想做事"的输入层事实，
/// 时间线靠它把玩家那条还没到点的行动撤掉，好让新的这一手抢到决策槽。
/// 右键不必写——它本来就是一条撤销请求。
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
    mut answers: MessageWriter<ReactionAnswer>,
    mut takeovers: MessageWriter<PlayerTakeover>,
) {
    for click in clicks.read() {
        match click {
            PointerCommand::Secondary => {
                undos.write(UndoCommand);
                // 右键同时是"放弃这一轮反制"：有反应窗口时它就是表态通道，
                // 没有窗口时这条消息自然被忽略（消费方按窗口存在与否判定）。
                // 这样输入域不必去读游戏状态。
                answers.write(ReactionAnswer::Abandon);
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
                takeovers.write(PlayerTakeover);
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
        takeovers: usize,
    }

    #[allow(clippy::too_many_arguments)]
    fn probe_system(
        mut probes: ResMut<Probes>,
        mut moves: MessageReader<MoveToCommand>,
        mut skills: MessageReader<UseSelectedSkill>,
        mut undos: MessageReader<UndoCommand>,
        mut takeovers: MessageReader<PlayerTakeover>,
    ) {
        probes.moves += moves.read().count();
        probes.skills += skills.read().count();
        probes.undos += undos.read().count();
        probes.takeovers += takeovers.read().count();
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
            .add_message::<PlayerTakeover>()
            .add_message::<ReactionAnswer>()
            .add_systems(Update, (pointer_command_system, probe_system).chain());
        app
    }

    /// 左键的两种去向：空地板 → 移动；有单位 → 用当前技能（没有"确认"这一步）。
    ///
    /// 两者都算「玩家自己动手了」；右键不算——它本来就是一条撤销请求。
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
        assert_eq!(probes.takeovers, 1, "左键算「玩家动手了」");

        // ② 那格上站着单位
        let mut app = click_app();
        app.insert_resource(HoveredCell(Some(cell)));
        app.world_mut()
            .spawn((cell, Faction::Enemy, Transform::default()));
        app.world_mut().write_message(PointerCommand::Primary);
        app.update();
        let probes = app.world().resource::<Probes>();
        assert_eq!((probes.moves, probes.skills), (0, 1), "点单位 = 用当前技能");
        assert_eq!(probes.takeovers, 1, "左键算「玩家动手了」");
    }

    /// 右键 = 撤销。
    #[test]
    fn secondary_click_asks_for_undo() {
        let mut app = click_app();
        app.world_mut().write_message(PointerCommand::Secondary);
        app.update();

        let probes = app.world().resource::<Probes>();
        assert_eq!((probes.undos, probes.moves), (1, 0), "右键只请求撤销");
        assert_eq!(probes.takeovers, 0, "撤销本身就是改主意，不必再多写一条");
    }

    /// 预演读数带上**速度帧**：`AttackFrame` 的消费者就是它（"谁先动"的洞察力读数）。
    #[test]
    fn the_preview_readout_carries_the_speed_frame() {
        use crate::presentation::hud::PreviewReadout;

        #[derive(Resource, Default)]
        struct Last(Option<String>);
        fn capture(mut readouts: MessageReader<PreviewReadout>, mut last: ResMut<Last>) {
            last.0 = readouts.read().last().and_then(|r| r.0.clone());
        }

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TerrainConfig::default())
            .insert_resource(MenuSelection::default())
            .init_resource::<HoveredCell>()
            .init_resource::<Last>()
            .add_message::<PreviewReadout>()
            .add_systems(Update, (update_preview_readout_system, capture).chain());
        // 玩家在原点，悬停一格之内的近处 → 「攻击」会派发成近战（frame 5）
        app.world_mut().spawn((
            crate::combat::Faction::Player,
            Transform::from_xyz(1.0, 0.0, 1.0),
        ));
        app.world_mut().resource_mut::<HoveredCell>().0 = Some(Cell::new(1, 0));
        app.update();

        let text = app
            .world()
            .resource::<Last>()
            .0
            .clone()
            .expect("应当有读数");
        assert!(
            text.contains("frame"),
            "预演读数应当带上速度帧（`AttackFrame` 的消费者），实际 {text:?}"
        );
    }
}
