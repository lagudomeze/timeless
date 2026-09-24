//! 时间轴悬停读数：滑到色块上，看这一手**是什么、打哪儿、还剩多久、能不能被打断**。
//!
//! 这一层只回答一件事：**把已经挂在行动实体上的那几个数拼成一行字**
//! （[`readout_line`] 是纯函数，可脱离 App 单测）。取数在 [`super::system`]，
//! 建 UI 在 [`super::scene`]，写 UI 也在 [`super::system`]——三层照旧。
//!
//! ## 为什么不需要任何新数据类型
//!
//! 要显示的每一个数**都已经存在**，只是玩家看不见（`docs/insight.md` 第一节）：
//!
//! | 读数 | 来自 |
//! | :--- | :--- |
//! | 谁 | 行动实体的归属（[`ActionOf`](crate::timeline::ActionOf)）→ 行动者的 `Faction` |
//! | 什么 | 载荷标记 → [`payload_name`](crate::presentation::hud::actions::payload_name) |
//! | 打哪儿 | `TargetCell`（火球锁的格）/ `Threatens`（威胁到哪些格） |
//! | 还剩多久 | [`ScheduledAction::execute_at`](crate::timeline::ScheduledAction) − `now` |
//! | 能不能被打断 | `CombatTags`（`interruptible` / `super_armor`） |
//!
//! 这条读数是「信息即力量」（`docs/game-design.md`）的第一块兑现：
//! 时间轴本来只回答"**什么时候**动手"，加上落点与打断抗性之后，
//! 同一个色块能回答"**我该走、该躲，还是该抢一手打掉它**"。

use bevy::prelude::*;

use crate::combat::Faction;
use crate::movement::Cell;
use crate::skills::CombatTags;

/// 悬停到某一条行动上时的全部读数（一帧的快照，纯数据）。
#[derive(Debug, Clone, PartialEq)]
pub struct ActionReadout {
    /// 这一手是谁的
    pub faction: Faction,
    /// 载荷名（`fireball` / `move` / `roll`…）
    pub payload: &'static str,
    /// 落点 / 威胁到的格（`TargetCell` 或 `Threatens`；没有就是 `None`）
    pub target: Option<Cell>,
    /// 前摇剩余秒数（`<= 0` = 已经落地）
    pub remaining: f32,
    /// 对抗标签：能不能被打断
    pub tags: CombatTags,
}

/// 一行读数：`ENEMY fireball → cell (3,1) · 0.4s · interruptible`。
///
/// 「还剩多久」与「能不能被打断」是玩家真正要预读的两个数——
/// 前者决定"来不来得及走"，后者决定"该走还是该抢一手"。
pub fn readout_line(readout: &ActionReadout) -> String {
    let name = match readout.faction {
        Faction::Player => "PLAYER",
        Faction::Enemy => "ENEMY",
    };
    let mut line = format!("{name} · {}", readout.payload);
    if let Some(cell) = readout.target {
        line.push_str(&format!(" → cell ({},{})", cell.x, cell.z));
    }
    if readout.remaining > 0.0 {
        line.push_str(&format!(" · {:.1}s", readout.remaining));
    }
    line.push_str(&format!(" · {}", interrupt_label(readout.tags)));
    line
}

/// 对抗标签 → 一行字（**闸门口径**，与 `interrupt_observer` 的判据一致）。
///
/// 判据只有一条：`!interruptible || super_armor` = 断不掉。与打断系统共用同一条
/// 口径很重要——读数说"能打断"而实际断不掉（或反过来），比没有读数更糟。
pub fn interrupt_label(tags: CombatTags) -> &'static str {
    if !tags.interruptible || tags.super_armor {
        "super armor"
    } else {
        "interruptible"
    }
}

/// 色块被悬停时记下的那一手（读数与高亮共用）。
///
/// 色块自身只有 `lane` / `slot`；"这一手是谁的、什么载荷"只有行动实体知道，
/// 所以悬停系统把它们摘出来放这儿，供其它表现层消费者读。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoveredAction {
    /// 这一手是哪个行动实体
    pub action: Entity,
    /// 这一手是谁的（战场高亮要圈住的就是他）
    pub actor: Entity,
}

/// 本帧鼠标停在哪个色块上（`None` = 没停在时间轴上）。
///
/// **这是本帧的事实快照，不是游戏状态**：它只在表现层内部传递，
/// 不写回任何领域数据（`docs/insight.md` 第三节）。
///
/// ⚠️ **悬停时间轴时鼠标不在战场上**，所以战场高亮有**两个来源**
/// （鼠标位置 `HoveredCell` / 时间轴 [`TimelineHover`]）。合并的口径按
/// `docs/insight.md` 第三节：**时间轴压过战场**——玩家的注意力在那里。
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct TimelineHover(pub Option<HoveredAction>);

/// 战场上「这一手是谁的」指示圈（开局生成一个，之后只搬位置 / 开关显隐）。
#[derive(Component, Reflect, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct TimelineFocusRing;

/// 读数条的根节点（默认隐藏）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct TimelineReadout;

/// 读数条的正文。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct TimelineReadoutText;

#[cfg(test)]
mod tests {
    use super::*;

    fn readout(tags: CombatTags, target: Option<Cell>, remaining: f32) -> ActionReadout {
        ActionReadout {
            faction: Faction::Enemy,
            payload: "fireball",
            target,
            remaining,
            tags,
        }
    }

    /// 一行读数里那五个字段都得在，而且顺序是"谁 · 什么 → 哪 · 还有多久 · 能不能打断"。
    #[test]
    fn the_line_carries_who_what_where_how_long_and_how_to_answer() {
        let line = readout(CombatTags::STRIKE, Some(Cell::new(3, 1)), 0.4).pipe_readout_line();
        assert!(line.starts_with("ENEMY · fireball"), "{line}");
        assert!(line.contains("cell (3,1)"), "落点要在读数里：{line}");
        assert!(line.contains("0.4s"), "剩余时间要在读数里：{line}");
        assert!(line.contains("interruptible"), "对抗标签要在读数里：{line}");
    }

    /// **霸体的读法与闸门口径一致**：`super_armor` 的一手显示 `super armor`。
    ///
    /// 这条守的是"读数不许和实际判定分叉"——读数说能打断、实际断不掉，
    /// 比没有读数更糟（玩家会照着错的读数做决定）。
    #[test]
    fn the_interrupt_readout_matches_the_gate() {
        assert_eq!(interrupt_label(CombatTags::STRIKE), "interruptible");
        assert_eq!(
            interrupt_label(CombatTags {
                super_armor: true,
                ..CombatTags::STRIKE
            }),
            "super armor",
            "霸体是「打断不了」，不是「比较难打断」"
        );
        assert_eq!(
            interrupt_label(CombatTags {
                interruptible: false,
                ..CombatTags::STRIKE
            }),
            "super armor",
            "不可打断与霸体在读数上是同一件事（都断不掉）"
        );
    }

    /// 没有落点（移动 / 跳跃 / 招架）与已经落地（`remaining <= 0`）时那两段**整段不出现**，
    /// 不留一个空的 `→ cell` 或 `· 0.0s`。
    #[test]
    fn empty_fields_are_left_out_entirely() {
        let line = readout(CombatTags::COMMITTED, None, 0.0).pipe_readout_line();
        assert!(!line.contains("cell"), "没有落点就不该出现落点段：{line}");
        assert!(
            !line.contains("0.0s") && !line.contains("· 0."),
            "已经落地就不该出现倒计时：{line}"
        );
        assert!(line.ends_with("super armor"), "{line}");
    }

    /// 读数的站位：**这一手还能改**（前摇中）才显示倒计时。
    #[test]
    fn the_remaining_seconds_only_show_while_the_windup_is_still_pending() {
        // 0.25 → 显示一位小数（格式化后是 0.2，不是四舍五入的 0.3）
        let pending = readout(CombatTags::STRIKE, None, 0.25).pipe_readout_line();
        assert!(pending.contains("0.2s"), "倒计时显示一位小数：{pending}");
        let landed = readout(CombatTags::STRIKE, None, 0.0).pipe_readout_line();
        assert!(!landed.contains("0.0s"), "{landed}");
    }

    /// 测试里少一层括号：让上面几条读起来像一句话。
    trait Pipe {
        fn pipe_readout_line(self) -> String;
    }
    impl Pipe for ActionReadout {
        fn pipe_readout_line(self) -> String {
            readout_line(&self)
        }
    }
}
