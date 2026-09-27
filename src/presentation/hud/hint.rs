//! 提示条：玩家面板正上方的一行字——**三种来源共用，按优先级取一个**。
//!
//! | 优先级 | 来源 | 时序 |
//! | :--- | :--- | :--- |
//! | 1（最高） | 被拒的输入：`ActionBlocked` / `BlockRefused` / `MoveRefused` / `EquipmentRefused` | 一次性，2 秒淡出 |
//! | 2 | **威胁窗口读数**（[`threat_hint`]，见 `docs/backlog/combat.md` 的 #53） | **持续**：窗口开着就一直显示 |
//! | 3（最低） | 预演读数 `PreviewReadout`（悬停格 → 技能 / 距离 / 预计伤害） | 跟着悬停 |
//!
//! **威胁读数与另外两种的关键差别**：它是**持续状态**，不参与淡出——
//! 世界冻着等玩家表态时，"这次只能躲"这句话不该两秒后自己消失。
//! 关窗即隐藏（表现层**只读** `ReactionSlot`，不自己判断"还有没有威胁"，
//! 否则两个地方各有一套判据迟早分叉）。
//!
//! ## 分两层：取数 → 写 UI
//!
//! [`gather_hint_system`] 把四个消息源 + 威胁窗口折成一份 [`HintState`] 快照，
//! [`apply_hint_system`] 只管把快照写进节点 + 记时。分开的两个理由：
//! ① 「三种来源谁压谁」这条判据可以脱开 UI 直接读资源断言；
//! ② 这个文件不再同时回答"取数 / 计时 / 配色 / 写节点"四件事。
//!
//! 为什么不复用别的区域：技能 tooltip 在正下方居中、战斗日志在右下，都会和它抢位置；
//! 锚在玩家面板上方既空着、又和「玩家现在能不能动」这件事最近。

use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};

use crate::combat::Faction;
use crate::combat::reaction::{ReactionSlot, TargetCell, ThreatKind, Threatens};
use crate::equipment::EquipmentRefused;
use crate::movement::{Cell, MoveRefused};
use crate::timeline::{ActionBlocked, ActionOf, BlockReason, ScheduledAction};
use crate::world::BlockRefused;

use super::actions::PayloadQueries;
use super::hud_text_tinted;

/// 「无法操作」提示停留时长（真实秒）：够读完一句短语，又不至于糊在屏幕上。
pub const HINT_SECS: f32 = 2.0;

/// 提示条根节点（默认隐藏）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct ActionHint;

/// 提示正文。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct ActionHintText;

/// 提示的语气：决定配色，也决定**要不要淡出**。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum HintKind {
    /// 预演读数（信息性质，跟着悬停）
    #[default]
    Info,
    /// 被拒的输入（警告性质，2 秒后自己消失）
    Warn,
    /// **威胁窗口**（警告性质，但**不淡出**——窗口开着就一直在）
    Threat,
}

/// 这一帧提示条该写什么。
///
/// 一个资源装三种来源的结果：**空正文 = 什么都不显示**（不用 `Option`：
/// 那条判据只在一个地方读，而 `Option<String>` 会让"没消息"与"空消息"
/// 变成两种要分辨的情况）。
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct HintState {
    pub text: String,
    pub kind: HintKind,
}

impl HintState {
    /// 有没有要显示的东西。
    pub fn is_showing(&self) -> bool {
        !self.text.is_empty()
    }
}

/// 预演读数（写：`interaction` 的悬停系统；消费：[`gather_hint_system`]）。
///
/// `None` = 没有预演目标（隐藏读数）；`Some(text)` = 显示"技能 · 距离 · 预计伤害"。
/// 和"无法操作"共用同一条提示条：被拒的输入优先级更高（它是即时反馈）。
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct PreviewReadout(pub Option<String>);

/// 提示条：玩家面板顶边（118px）再往上 8px，默认隐藏。
pub fn hint_panel(font: &Handle<Font>) -> impl Bundle {
    (
        Name::new("ActionHint"),
        ActionHint,
        // 吃掉指针：提示条是给玩家读的，压在上面时不该把点击滤到地面
        FocusPolicy::Block,
        RelativeCursorPosition::default(),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(14.0),
            bottom: Val::Px(126.0),
            display: Display::None,
            ..default()
        },
        children![(
            Name::new("ActionHintText"),
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(COLOR_WARN_BG),
            hud_text_tinted(font, 12.0, "", COLOR_WARN_TEXT),
            ActionHintText,
        )],
    )
}

/// 威胁窗口的一句话读数（**纯函数**，可脱离 App 单测）。
///
/// 同一次攻击会问玩家两次（前摇中 / 出手后），两档文案必须**读得出差别**——
/// 这正是 #53 的核心：以前两次一模一样，玩家不知道该按什么。
///
/// - `Incoming`：来源还在前摇，**可以打断 / 反制**，所以给"还有多久落地"；
/// - `InFlight`：来源已经出手，**只能躲或忍**，所以给"落点在哪"。
///
/// 键位写玩家看得到的那个（`E` = 翻滚，与 `HELP_LINES` 由测试对账）。
pub fn threat_hint(
    kind: ThreatKind,
    attacker: Faction,
    payload: &str,
    landing: Option<Cell>,
    remaining: Option<f32>,
) -> String {
    let side = if attacker == Faction::Player {
        "友方"
    } else {
        "敌"
    };
    match kind {
        ThreatKind::Incoming => {
            let eta = remaining.map_or(String::new(), |left| format!("（{left:.1}s 后落地）"));
            format!("{side} {payload} 锁定你{eta} · 可打断（E 翻滚躲 / 右键忍）")
        }
        ThreatKind::InFlight => {
            let landing = landing.map_or(String::new(), |cell| {
                format!(" → 落点 ({},{})", cell.x, cell.z)
            });
            format!("{side} {payload} 已出手{landing}（E 翻滚躲 / 右键忍）")
        }
    }
}

/// 取数：四个"不让做"的消息源 + 威胁窗口 + 预演读数 → 一份 [`HintState`]。
///
/// **优先级**：被拒的输入 > 威胁窗口 > 预演读数（只在这里判一次，别处不再排一遍）。
#[allow(clippy::too_many_arguments)]
pub fn gather_hint_system(
    mut blocked: MessageReader<ActionBlocked>,
    mut refused: MessageReader<BlockRefused>,
    mut moves_refused: MessageReader<MoveRefused>,
    mut equipment_refused: MessageReader<EquipmentRefused>,
    mut readouts: MessageReader<PreviewReadout>,
    slots: Query<&ReactionSlot>,
    threats: Query<(&Threatens, &ScheduledAction, &ActionOf)>,
    projectiles: Query<(&TargetCell, &Faction)>,
    actors: Query<&Faction>,
    payloads: PayloadQueries<'_, '_>,
    now: Res<Time<Virtual>>,
    mut state: ResMut<HintState>,
) {
    // ① 这一帧有没有"不让做"——四种原因汇到一条提示条，文案的构造只有一处
    let move_refused = moves_refused
        .read()
        .last()
        .map(|MoveRefused::BlockedByTerrain| "BLOCKED · too high to step up".to_string());
    let refused_message = refused
        .read()
        .last()
        .map(|BlockRefused::TerrainNotLoaded| "NO GROUND HERE · chunk not loaded".to_string())
        .or(move_refused);
    let equipment_message = equipment_refused
        .read()
        .last()
        .map(|refused| match refused {
            EquipmentRefused::WrongSlot { item, slot } => {
                format!("WON'T FIT · {} in the {} slot", item.label(), slot.label())
            }
            EquipmentRefused::NoSuchSlot => "NO SUCH SLOT".to_string(),
        });
    let blocked_message = blocked.read().last().map(|last| match last.reason {
        BlockReason::Busy => "CAN'T ACT YET · still busy".to_string(),
        BlockReason::NotEnoughEnergy => "NOT ENOUGH ENERGY".to_string(),
        BlockReason::NotEnoughAmmo => "NOT ENOUGH AMMO".to_string(),
    });

    let next = if let Some(text) = equipment_message.or(refused_message).or(blocked_message) {
        HintState {
            text,
            kind: HintKind::Warn,
        }
    } else if let Some(text) = current_threat_hint(
        &slots,
        &threats,
        &projectiles,
        &actors,
        &payloads,
        now.elapsed_secs(),
    ) {
        // ② 威胁窗口：**持续**，所以它不走计时器
        HintState {
            text,
            kind: HintKind::Threat,
        }
    } else {
        // ③ 预演读数：跟着悬停走
        match readouts.read().last() {
            Some(PreviewReadout(Some(text))) => HintState {
                text: text.clone(),
                kind: HintKind::Info,
            },
            _ => HintState::default(),
        }
    };

    if *state != next {
        *state = next;
    }
}

/// 当前窗口该写的那句话（没有窗口 / 已表态 → `None`）。
///
/// `kind` 与 `threat` 都是**开窗时写好的**：读数层不重新判断
/// "这是前摇还是已经出手"——那会与 `detect_threat_system` 分叉。
fn current_threat_hint(
    slots: &Query<&ReactionSlot>,
    threats: &Query<(&Threatens, &ScheduledAction, &ActionOf)>,
    projectiles: &Query<(&TargetCell, &Faction)>,
    actors: &Query<&Faction>,
    payloads: &PayloadQueries<'_, '_>,
    now: f32,
) -> Option<String> {
    // 窗口挂在被威胁的**玩家**身上；`resolved`（表过态）之后不再读数
    let slot = slots.iter().find(|slot| !slot.resolved)?;
    let threat = slot.threat;
    let kind = slot.kind;

    // 出手方：投射物自己的 `Faction`，行动实体则要看它的行动者
    let attacker = actors
        .get(threat)
        .ok()
        .copied()
        .or_else(|| {
            threats
                .get(threat)
                .ok()
                .and_then(|(_, _, action_of)| actors.get(action_of.actor()).ok().copied())
        })
        .unwrap_or(Faction::Enemy);
    let payload = payloads.name_of(threat);
    // 落点：已经出手的看 `TargetCell`，还在前摇的看它声明威胁的最后一格
    let landing = projectiles
        .get(threat)
        .ok()
        .map(|(target, _)| target.0)
        .or_else(|| {
            threats
                .get(threat)
                .ok()
                .and_then(|(cells, _, _)| cells.cells.last().copied())
        });
    // 还剩多久落地：只有前摇中的行动有 `execute_at`
    let remaining = threats
        .get(threat)
        .ok()
        .map(|(_, schedule, _)| (schedule.execute_at - now).max(0.0));

    Some(threat_hint(kind, attacker, payload, landing, remaining))
}

/// 写 UI：把 [`HintState`] 落到提示条上（内容没变就整帧不碰节点）。
///
/// **计时只作用于被拒的输入**：预演读数跟着悬停、威胁读数跟着窗口，
/// 两者都不该因为"过了两秒"而消失。
pub fn apply_hint_system(
    time: Res<Time<Real>>,
    mut timer: ResMut<HintTimer>,
    state: Res<HintState>,
    mut nodes: Query<&mut Node, With<ActionHint>>,
    mut texts: Query<(&mut Text, &mut TextColor, &mut BackgroundColor), With<ActionHintText>>,
) {
    if !state.is_showing() {
        timer.0 = 0.0;
        for mut node in &mut nodes {
            node.display = Display::None;
        }
        return;
    }

    if state.kind == HintKind::Warn {
        timer.0 = HINT_SECS;
    }
    if timer.0 > 0.0 {
        timer.0 = (timer.0 - time.delta_secs()).max(0.0);
    }

    for (mut text, mut color, mut background) in &mut texts {
        if **text != state.text {
            **text = state.text.clone();
        }
        let warn = state.kind != HintKind::Info;
        *color = TextColor(if warn {
            COLOR_WARN_TEXT
        } else {
            COLOR_INFO_TEXT
        });
        *background = BackgroundColor(if warn { COLOR_WARN_BG } else { COLOR_INFO_BG });
    }
    for mut node in &mut nodes {
        node.display = Display::Flex;
    }
}

/// 「无法操作」提示剩余时间（真实秒）；`<= 0` 表示不显示。
#[derive(Resource, Debug, Default)]
pub struct HintTimer(pub f32);

/// 「无法操作」的文字 / 底色（偏暖，提醒性质）。
const COLOR_WARN_TEXT: Color = Color::srgb(1.0, 0.88, 0.80);
/// 见 [`COLOR_WARN_TEXT`]。
const COLOR_WARN_BG: Color = Color::srgba(0.10, 0.12, 0.17, 0.92);
/// 预演读数的文字 / 底色（偏冷，信息性质）。
const COLOR_INFO_TEXT: Color = Color::srgb(0.86, 0.92, 1.0);
/// 见 [`COLOR_INFO_TEXT`]。
const COLOR_INFO_BG: Color = Color::srgba(0.06, 0.08, 0.12, 0.92);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::{ActionOf, ActionTiming, ScheduledAction};
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    fn hint_app() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .init_resource::<HintTimer>()
            .init_resource::<HintState>()
            .add_message::<ActionBlocked>()
            .add_message::<BlockRefused>()
            .add_message::<MoveRefused>()
            .add_message::<crate::equipment::EquipmentRefused>()
            .add_message::<PreviewReadout>()
            .add_systems(Update, (gather_hint_system, apply_hint_system).chain());
        let node = app
            .world_mut()
            .spawn((
                ActionHint,
                Node {
                    display: Display::None,
                    ..default()
                },
            ))
            .id();
        let text = app
            .world_mut()
            .spawn((ActionHintText, Text::new(""), TextColor(Color::WHITE)))
            .id();
        (app, node, text)
    }

    /// 被拒的输入要看得见，而且**在冻结的世界里也会自己消失**。
    #[test]
    fn a_blocked_input_shows_a_message_that_fades_out() {
        let (mut app, node, text) = hint_app();
        app.world_mut().write_message(ActionBlocked::BUSY);
        app.update();

        assert_eq!(
            app.world().get::<Node>(node).unwrap().display,
            Display::Flex,
            "被拒的输入应当让提示显示出来"
        );
        let shown = app.world().get::<Text>(text).unwrap().0.clone();
        assert!(
            shown.contains("CAN'T ACT"),
            "文案要说清楚是「还动不了」而不是别的：{shown}"
        );

        // 2.0s 之后自己消失（每帧 0.1s，第一帧 dt=0，所以跑够 30 帧）
        for _ in 0..30 {
            app.update();
        }
        assert_eq!(
            app.world().get::<Node>(node).unwrap().display,
            Display::None,
            "提示到点应当自己隐藏，不需要玩家做任何事"
        );
    }

    /// 方块交互被拒也走同一条提示条——`world` 只宣布原因，文案由表现层给。
    #[test]
    fn a_refused_block_edit_reaches_the_same_hint_bar() {
        let (mut app, node, text) = hint_app();
        app.world_mut()
            .write_message(BlockRefused::TerrainNotLoaded);
        app.update();

        assert_eq!(
            app.world().get::<Node>(node).unwrap().display,
            Display::Flex
        );
        assert!(
            app.world().get::<Text>(text).unwrap().0.contains("GROUND"),
            "{}",
            app.world().get::<Text>(text).unwrap().0
        );
    }

    /// 走不过去也走同一条提示条：地形很高时玩家该看得见"为什么不动"。
    #[test]
    fn a_refused_step_reaches_the_same_hint_bar() {
        let (mut app, node, text) = hint_app();
        app.world_mut().write_message(MoveRefused::BlockedByTerrain);
        app.update();

        assert_eq!(
            app.world().get::<Node>(node).unwrap().display,
            Display::Flex
        );
        assert!(
            app.world().get::<Text>(text).unwrap().0.contains("BLOCKED"),
            "{}",
            app.world().get::<Text>(text).unwrap().0
        );
    }

    /// 精力不足是另一种原因，文案不同。
    #[test]
    fn running_out_of_energy_gets_its_own_message() {
        let (mut app, node, text) = hint_app();
        app.world_mut().write_message(ActionBlocked::NO_ENERGY);
        app.update();

        assert_eq!(
            app.world().get::<Node>(node).unwrap().display,
            Display::Flex
        );
        assert!(
            app.world().get::<Text>(text).unwrap().0.contains("ENERGY"),
            "{}",
            app.world().get::<Text>(text).unwrap().0
        );
    }

    /// **两档威胁文案读得出差别**（#53 的核心）。
    ///
    /// 以前两次问询长得一模一样，玩家不知道该按什么：
    /// 前摇中还能打断 / 反制，出手后只能躲或忍。
    #[test]
    fn the_two_threat_readouts_say_different_things() {
        let incoming = threat_hint(
            ThreatKind::Incoming,
            Faction::Enemy,
            "fireball",
            Some(Cell::new(3, 1)),
            Some(0.4),
        );
        assert!(incoming.contains("敌"), "{incoming}");
        assert!(incoming.contains("fireball"), "{incoming}");
        assert!(
            incoming.contains("可打断"),
            "前摇中要告诉玩家还能打断：{incoming}"
        );
        assert!(
            incoming.contains("0.4"),
            "前摇中要给「还有多久落地」：{incoming}"
        );
        assert!(
            incoming.contains("翻滚") && incoming.contains("右键"),
            "两条出路都要写出来：{incoming}"
        );

        let in_flight = threat_hint(
            ThreatKind::InFlight,
            Faction::Enemy,
            "fireball",
            Some(Cell::new(3, 1)),
            None,
        );
        assert!(in_flight.contains("已出手"), "{in_flight}");
        assert!(
            in_flight.contains("(3,1)"),
            "已经出手要给**落点**（往旁边一步就安全）：{in_flight}"
        );
        assert_ne!(
            incoming, in_flight,
            "两档文案必须不同——一样的话玩家读不出这次该按什么"
        );
    }

    /// 出手方是玩家时写「友方」：文案不该把玩家的火球说成敌人的。
    #[test]
    fn a_friendly_threat_is_not_called_an_enemy() {
        let text = threat_hint(
            ThreatKind::Incoming,
            Faction::Player,
            "fireball",
            None,
            Some(0.2),
        );
        assert!(text.starts_with("友方"), "{text}");
        assert!(!text.contains("敌 "), "不能把自己的火球说成敌方的：{text}");
    }

    /// **威胁读数不淡出**：窗口开着就一直显示（它是持续状态，不是一次性提示）。
    #[test]
    fn a_threat_readout_does_not_fade_away() {
        let (mut app, node, _) = hint_app();
        let player = app.world_mut().spawn_empty().id();
        let threat = app.world_mut().spawn(Faction::Enemy).id();
        app.world_mut().entity_mut(player).insert(ReactionSlot {
            threat,
            kind: ThreatKind::InFlight,
            suggestions: Vec::new(),
            resolved: false,
        });

        // 跑够 3 秒（远超 HINT_SECS = 2.0）
        for _ in 0..30 {
            app.update();
        }
        assert_eq!(
            app.world().get::<Node>(node).unwrap().display,
            Display::Flex,
            "威胁窗口开着时读数必须一直在——它靠窗口活着，不靠计时器"
        );

        // 关窗（表态）之后立刻收起来，不需要等计时器
        app.world_mut()
            .get_mut::<ReactionSlot>(player)
            .unwrap()
            .resolved = true;
        app.update();
        assert_eq!(
            app.world().get::<Node>(node).unwrap().display,
            Display::None,
            "表过态就该收起读数"
        );
    }

    /// **优先级：被拒的输入压过威胁读数**（它是即时反馈）。
    #[test]
    fn a_blocked_input_outranks_the_threat_readout() {
        let (mut app, _, text) = hint_app();
        let player = app.world_mut().spawn_empty().id();
        let threat = app.world_mut().spawn(Faction::Enemy).id();
        app.world_mut().entity_mut(player).insert(ReactionSlot {
            threat,
            kind: ThreatKind::Incoming,
            suggestions: Vec::new(),
            resolved: false,
        });
        app.update();
        assert!(
            app.world().get::<Text>(text).unwrap().0.contains("翻滚"),
            "先显示威胁读数"
        );

        app.world_mut().write_message(ActionBlocked {
            reason: BlockReason::NotEnoughAmmo,
        });
        app.update();
        assert!(
            app.world().get::<Text>(text).unwrap().0.contains("AMMO"),
            "被拒的输入要压过威胁读数：{}",
            app.world().get::<Text>(text).unwrap().0
        );
    }

    /// 威胁读数里的**倒计时跟着虚拟时钟**（与 `act:` 行同源）。
    #[test]
    fn the_threat_countdown_follows_the_virtual_clock() {
        let (mut app, _, text) = hint_app();
        let player = app.world_mut().spawn_empty().id();
        let actor = app.world_mut().spawn(Faction::Enemy).id();
        let action = app
            .world_mut()
            .spawn((
                Threatens {
                    cells: vec![Cell::new(0, 0)],
                },
                // 前摇 0.5s，`declared_at(4.5)` → `execute_at = 5.0`
                ScheduledAction::declared_at(ActionTiming::new(0.5, 1.0, 1), 4.5),
                ActionTiming::new(0.5, 1.0, 1),
                ActionOf(actor),
            ))
            .id();
        app.world_mut().entity_mut(player).insert(ReactionSlot {
            threat: action,
            kind: ThreatKind::Incoming,
            suggestions: Vec::new(),
            resolved: false,
        });

        app.update(); // dt = 0
        let first = app.world().get::<Text>(text).unwrap().0.clone();
        for _ in 0..5 {
            app.update(); // 0.5 虚拟秒
        }
        let later = app.world().get::<Text>(text).unwrap().0.clone();

        assert!(
            first.contains("5.0"),
            "第一帧的剩余时间应当接近 execute_at：{first}"
        );
        assert!(later.contains("4.5"), "跑 0.5 秒之后应当读到 4.5：{later}");
    }
}
