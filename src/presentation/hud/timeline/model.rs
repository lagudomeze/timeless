//! 时间轴的**模型**：把「谁在什么时候出手」算成一份纯数据快照。
//!
//! 这里没有任何 Bevy 系统、没有实体、没有 `Node`——输入是普通的切片，输出是
//! [`TimelineModel`]（车道 → 色块 + 候场名单）。因此"色块位置对不对""谁在候场"
//! 这类问题可以脱离 App 直接单测（本文件的 `tests` 就是这么做的）。
//!
//! 画出来是 [`super::scene`] 的事，写 UI 是 [`super::system`] 的事。
//!
//! 横轴是**虚拟秒**：左端 = 现在，右端 = [`WINDOW_SECONDS`] 之后。
//! **色块 = 占用，刻线 = 结算**（两种信息走两条视觉通道）：
//!
//! - 色块左边界 = **声明时刻**（`execute_at − windup`），宽度 = 前摇 + 后摇
//!   （见 [`crate::timeline::ActionTiming`]）——所以它天然从"现在"刻线长出去，
//!   回答"我按下去之后要忙多久、什么时候能再决策"；
//! - 块内那条白色竖线 = **结算时刻**（`execute_at` = 声明时刻 + 前摇），
//!   回答"我这一手什么时候真的发生 / 什么时候会挨打"。
//!
//! **每个单位一行（lane）**：一条横轴解决不了"谁在动手"——两个单位同时出手时色块会
//! 叠在一起。所以纵轴按单位分道：玩家在最上面一行，其余按稳定顺序往下排。
//! 没有排期的人不占横轴，而是站在**右侧候场区**：那里只显示"现在能决策、
//! 但还没声明"的人，一眼能看出还剩谁没动。
//!
//! 无回合模型里没有「回合格子」，所以时间轴**不是**回合队列，而是一段连续时间窗；
//! 玩家等输入时虚拟时间冻结，色块也跟着停住（这正是玩家读盘的时机）。

use bevy::prelude::*;

use crate::combat::Faction;

/// 时间轴视野（虚拟秒）：只画这段时间里会发生的行动。
pub const WINDOW_SECONDS: f32 = 4.0;
/// 刻度间隔（虚拟秒）。条子在冻结时是不动的，刻度就是它唯一的"尺子"。
pub const TICK_SECONDS: f32 = 0.5;
/// 刻度数量（`0` 处是"现在"刻线，不重复画）。
pub const TICK_POOL: usize = (WINDOW_SECONDS / TICK_SECONDS) as usize;
/// 色块最小宽度（百分比），免得极短动作看不见。
pub const MIN_BLOCK_WIDTH: f32 = 3.0;
/// 车道池大小（每个单位一行；常驻 1 玩家 + 1 敌人，留几个空位给召唤物）。
pub const LANE_POOL: usize = 4;
/// 单条车道的色块池：一个单位同一时刻只会有一个未落地的行动。
pub const BLOCK_POOL_PER_LANE: usize = 2;
/// 单条车道的高度（像素）。
pub const LANE_HEIGHT: f32 = 12.0;
/// 车道之间的竖直间隙（像素）。
pub const LANE_GAP: f32 = 2.0;
/// 右侧候场区宽度（像素）。
pub const STAGING_WIDTH: f32 = 24.0;

/// 一个色块的「时间轴坐标 + 归属」（纯数据，用来比对是否需要重画）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimelineSlot {
    pub left: f32,
    pub width: f32,
    /// 结算刻线在块内的位置（%）：前摇 / 总时长
    pub mark: f32,
    pub faction: Faction,
    pub draft: bool,
}

/// 一条待排期的行动：从行动实体上摘下来的几个数（不持有实体引用之外的东西）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActionRow {
    /// 行动者
    pub actor: Entity,
    /// 声明时刻（= `execute_at − windup`）
    pub declared_at: f32,
    /// 前摇 + 后摇 = 这个单位被占住的总时长
    pub total: f32,
    /// 前摇（决定结算刻线落在块内哪里）
    pub windup: f32,
    /// 还没到点 = 还撤得掉（半透明表示"这一手还改得动"）
    pub draft: bool,
}

/// 一帧的时间轴快照：状态行 + 每条车道的色块 + 候场名单。
///
/// 这才是**写进缓存**的东西：与上一帧完全相等就整帧不碰 UI。
/// [`TimelineModel`] 多带的 `lane_faction` 不进缓存——它只用来写 UI，
/// 车道内容本身（`lanes` / `ready` / `state`）一变就整批重写。
/// 冻结时这条路径收益最大（虚拟时间冻结、`now` 不变、队列不变），
/// 每帧的 `Node` 写入全部省掉。
#[derive(Debug, Default, Clone, PartialEq)]
pub struct TimelineCache {
    state: String,
    /// 每条车道的色块（下标 = lane）
    lanes: Vec<Vec<TimelineSlot>>,
    /// 站在候场区的车道（已就绪、还没声明）
    ready: Vec<usize>,
}

/// 一帧的时间轴快照 = 可缓存的部分 + 写 UI 要用的阵营表。
#[derive(Debug, Default, Clone, PartialEq)]
pub struct TimelineModel {
    /// 状态行文案
    pub state: String,
    /// 每条车道的色块（下标 = lane）
    pub lanes: Vec<Vec<TimelineSlot>>,
    /// 站在候场区的车道（已就绪、还没声明）
    pub ready: Vec<usize>,
    /// 每条车道的阵营（写 UI 的字母与颜色都要它）
    pub lane_faction: Vec<Option<Faction>>,
}

impl TimelineModel {
    /// 把可缓存的那几项写进 [`TimelineCache`]。
    pub fn store(&self, cache: &mut TimelineCache) {
        cache.state.clone_from(&self.state);
        cache.lanes.clone_from(&self.lanes);
        cache.ready.clone_from(&self.ready);
    }

    /// 与缓存快照完全一致（= 这一帧不必碰 UI）。
    pub fn matches(&self, cache: &TimelineCache) -> bool {
        cache.state == self.state && cache.lanes == self.lanes && cache.ready == self.ready
    }
}

/// 车道排序权重：玩家永远在最上面一行，其余按阵营稳定排。
pub fn faction_rank(faction: Faction) -> u8 {
    match faction {
        Faction::Player => 0,
        Faction::Enemy => 1,
    }
}

/// 阵营字母（色块 / 车道 / 候场方块共用）。
pub fn faction_letter(faction: Faction) -> &'static str {
    match faction {
        Faction::Player => "P",
        Faction::Enemy => "E",
    }
}

/// 行动在时间轴上的（左边界 %, 宽度 %）；不在视野内返回 `None`。
///
/// `declared_at` 是**声明时刻**（不是执行时刻）：宽度 = 前摇 + 后摇 = 这一整段
/// 「该单位被占住」的时间，所以色块从"现在"长出去；结算点画在块内
/// [`resolve_mark_percent`] 的位置。
///
/// 已经开始的行动（`declared_at < now`，例如正在飞的火球）裁到左端而不是消失——
/// 玩家仍然看得见「它还在进行中」。
pub fn block_span(declared_at: f32, total: f32, now: f32, window: f32) -> Option<(f32, f32)> {
    if total <= 0.0 || window <= 0.0 {
        return None;
    }
    let start = declared_at - now;
    let end = start + total;
    if end <= 0.0 || start > window {
        return None;
    }
    let left = (start.max(0.0) / window) * 100.0;
    let right = (end.min(window) / window) * 100.0;
    Some((left, (right - left).max(MIN_BLOCK_WIDTH)))
}

/// 结算刻线在色块内的位置（%）：前摇占整段忙时间的比例。
///
/// `total = 前摇 + 后摇`，所以它永远落在块内（0% = 声明即结算，100% = 全在等结算）。
pub fn resolve_mark_percent(windup: f32, total: f32) -> f32 {
    if total <= 0.0 {
        return 0.0;
    }
    (windup / total * 100.0).clamp(0.0, 100.0)
}

/// 把这一帧的世界算成一份快照。**纯函数**：不碰 `World`，只读切片。
///
/// - `frozen`：本帧冻结的原因（`None` = 时间在走），只影响状态行文案；
/// - `roster`：场上单位（顺序不限，这里按 `faction_rank` + 实体序号排稳）；
/// - `rows`：所有未落地的行动；
/// - `ready`：决策槽空着的单位（候场候选人）；
/// - `now`：当前虚拟秒。
pub fn build_model(
    frozen: Option<&[&str]>,
    roster: &[(Entity, Faction)],
    rows: &[ActionRow],
    ready: &[Entity],
    now: f32,
) -> TimelineModel {
    // 冻结的原因直接读出来：玩家一眼知道是"等我决策"、"我按了空格"还是"有人打过来"
    let state = match frozen {
        Some(labels) => format!("TIMELINE · FROZEN · {}", labels.join(" + ")),
        None => "TIMELINE · RUNNING".to_string(),
    };

    // 稳定的 actor → lane 映射：玩家排最上面，其余按实体序号（同帧可复现）
    let mut sorted: Vec<(Entity, Faction)> = roster.to_vec();
    sorted.sort_by_key(|(entity, faction)| (faction_rank(*faction), entity.index()));

    let lane_actor: Vec<Option<Entity>> = (0..LANE_POOL)
        .map(|lane| sorted.get(lane).map(|(entity, _)| *entity))
        .collect();
    let lane_faction: Vec<Option<Faction>> = (0..LANE_POOL)
        .map(|lane| sorted.get(lane).map(|(_, faction)| *faction))
        .collect();

    // 每个单位只画自己那一行
    let mut lanes: Vec<Vec<TimelineSlot>> = vec![Vec::new(); LANE_POOL];
    for row in rows {
        let Some(lane) = lane_actor
            .iter()
            .position(|actor| *actor == Some(row.actor))
        else {
            continue; // 行动者不在花名册里（刚销毁 / 还没组装）
        };
        let Some(faction) = lane_faction[lane] else {
            continue;
        };
        let Some((left, width)) = block_span(row.declared_at, row.total, now, WINDOW_SECONDS)
        else {
            continue;
        };
        lanes[lane].push(TimelineSlot {
            left,
            width,
            mark: resolve_mark_percent(row.windup, row.total),
            faction,
            draft: row.draft,
        });
    }
    for lane in &mut lanes {
        // 同一行的先后顺序稳定：按左边界
        lane.sort_by(|a, b| a.left.total_cmp(&b.left));
    }

    // 候场区：已就绪、还没有排期的人
    let ready_lanes: Vec<usize> = (0..LANE_POOL)
        .filter(|lane| {
            lane_actor[*lane].is_some_and(|actor| ready.contains(&actor)) && lanes[*lane].is_empty()
        })
        .collect();

    TimelineModel {
        state,
        lanes,
        ready: ready_lanes,
        lane_faction,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movement::MOVE_TIMING;

    #[test]
    fn block_span_maps_seconds_to_percent_of_the_window() {
        // 现在 = 0，动作在 1.0s 处声明、总共占 1.0s，视野 4s：左边界 25%、宽度 25%
        let (left, width) = block_span(1.0, 1.0, 0.0, 4.0).unwrap();
        assert_eq!((left, width), (25.0, 25.0));
    }

    /// 刚声明的动作必须**贴着"现在"刻线**长出去。
    ///
    /// 这是曾经的 bug：色块按 `execute_at` 画，于是整块右移了一个前摇
    /// （移动 0.25s 的块画在 [0.15, 0.40]），观感就是"一块东西在半空里跳"。
    #[test]
    fn a_fresh_action_starts_at_the_playhead() {
        let now = 7.5;
        let (left, width) =
            block_span(now, MOVE_TIMING.total(), now, WINDOW_SECONDS).expect("刚声明应当可见");
        assert_eq!(left, 0.0, "声明时刻 = 现在 → 左边界就落在刻线上");
        assert_eq!(width, MOVE_TIMING.total() / WINDOW_SECONDS * 100.0);
    }

    /// 结算刻线落在块内：移动的前摇占 0.15 / 0.25 = 60%。
    #[test]
    fn resolve_mark_sits_inside_the_block_at_the_windup_share() {
        let percent = resolve_mark_percent(MOVE_TIMING.windup, MOVE_TIMING.total());
        assert!(
            (percent - 60.0).abs() < 1e-3,
            "0.15 / 0.25 应当落在块的 60%，实际 {percent}"
        );
        assert_eq!(resolve_mark_percent(0.0, 1.0), 0.0, "零前摇 = 声明即结算");
        assert_eq!(resolve_mark_percent(1.0, 0.0), 0.0, "零时长不除零");
    }

    #[test]
    fn block_span_clips_actions_that_already_started() {
        // 1 秒前声明、总共占 2 秒的行动（比如正在飞的火球）裁到左端，而不是消失
        let (left, width) = block_span(0.0, 2.0, 1.0, 4.0).unwrap();
        assert_eq!(left, 0.0);
        assert_eq!(width, 25.0, "剩下的 1s 占 4s 视野的 25%");
    }

    #[test]
    fn block_span_drops_finished_and_far_actions() {
        assert!(block_span(0.0, 0.5, 1.0, 4.0).is_none(), "已经结束");
        assert!(block_span(9.0, 1.0, 1.0, 4.0).is_none(), "还在视野之外");
        assert!(block_span(1.0, 0.0, 0.0, 4.0).is_none(), "零时长不画");
    }

    /// 玩家永远在第一行，其余按实体序号往下排——同帧可复现。
    #[test]
    fn the_player_always_takes_the_top_lane() {
        let mut world = World::new();
        let enemy_first = world.spawn_empty().id();
        let enemy_second = world.spawn_empty().id();
        let player = world.spawn_empty().id();
        // 故意把玩家放在名单**后面**，排序权重要把它拉回第一行
        let roster = [
            (enemy_first, Faction::Enemy),
            (enemy_second, Faction::Enemy),
            (player, Faction::Player),
        ];

        let model = build_model(None, &roster, &[], &[], 0.0);

        assert_eq!(model.lane_faction[0], Some(Faction::Player));
        assert_eq!(model.lane_faction[1], Some(Faction::Enemy));
        assert_eq!(model.lane_faction[2], Some(Faction::Enemy));
        assert_eq!(model.lane_faction[3], None, "空位留 None");
    }

    /// 色块只进它自己那一行；已经落地的行动（视野之外）不进任何一行。
    #[test]
    fn each_actor_lands_in_its_own_lane() {
        let mut world = World::new();
        let player = world.spawn_empty().id();
        let enemy = world.spawn_empty().id();
        let roster = [(player, Faction::Player), (enemy, Faction::Enemy)];
        let rows = [
            ActionRow {
                actor: enemy,
                declared_at: 0.5,
                total: MOVE_TIMING.total(),
                windup: MOVE_TIMING.windup,
                draft: true,
            },
            ActionRow {
                actor: player,
                declared_at: 2.0,
                total: MOVE_TIMING.total(),
                windup: MOVE_TIMING.windup,
                draft: true,
            },
            // 完全在视野之外：不该出现
            ActionRow {
                actor: enemy,
                declared_at: 99.0,
                total: MOVE_TIMING.total(),
                windup: MOVE_TIMING.windup,
                draft: false,
            },
        ];

        let model = build_model(None, &roster, &rows, &[], 0.0);

        assert_eq!(model.lanes[0].len(), 1, "玩家行只有自己那一条");
        assert_eq!(model.lanes[1].len(), 1, "敌人行只有自己那一条");
        assert!(model.lanes[0][0].left > 25.0, "2.0s 声明 → 落在 50%");
        assert!(model.lanes[1][0].left < 25.0, "0.5s 声明 → 落在 12.5%");
        assert_ne!(
            model.lanes[0][0].faction, model.lanes[1][0].faction,
            "两条车道按阵营区分"
        );
    }

    /// 候场区：决策槽空着、且这一行没有排期的人才站上去。
    #[test]
    fn only_idle_actors_without_a_schedule_stand_in_staging() {
        let mut world = World::new();
        let idle = world.spawn_empty().id();
        let busy = world.spawn_empty().id();
        let roster = [(idle, Faction::Player), (busy, Faction::Enemy)];
        // busy 虽然决策槽也空着，但已经有一条排期 → 不该站候场
        let rows = [ActionRow {
            actor: busy,
            declared_at: 0.0,
            total: MOVE_TIMING.total(),
            windup: MOVE_TIMING.windup,
            draft: false,
        }];

        let model = build_model(None, &roster, &rows, &[idle, busy], 0.0);

        assert_eq!(model.ready, vec![0], "只有既空闲又没排期的人候场");
    }

    /// 冻结原因直接进状态行，玩家一眼看出是谁停的表。
    #[test]
    fn the_state_line_reads_the_freeze_reasons() {
        let mut world = World::new();
        let player = world.spawn_empty().id();
        let roster = [(player, Faction::Player)];

        let running = build_model(None, &roster, &[], &[], 0.0);
        assert_eq!(running.state, "TIMELINE · RUNNING");

        let labels: [&str; 2] = ["manual", "threat"];
        let frozen = build_model(Some(&labels), &roster, &[], &[], 0.0);
        assert_eq!(frozen.state, "TIMELINE · FROZEN · manual + threat");
    }
}
