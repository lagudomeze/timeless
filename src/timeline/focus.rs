//! ⚠️ **这一族不是时间线的概念**：`Focus` 是单位的**反制资源**（和
//! `combat::defense` 的 `Stamina` 同类）。它寄住在时间线里，只是因为
//! "把一次声明的前摇买掉"这件事发生在声明那一刻——而**声明归各领域**，
//! 时间线只提供 [`ScheduledAction::with_focus`](super::ScheduledAction::with_focus)
//! 这个共用入口。
//!
//! **`Focus` 是挂在单位身上的组件**，不是全局资源：**每个单位有自己的余量**。
//! 这就让 AI 也能用它——精英怪攒够 Focus 同样可以抢先手（见 [`crate::ai`]）。
//! 全局资源只有一份，那个形态下敌人永远不可能有 Focus。
//!
//! 本帧请求 [`PendingFocus`] 仍是资源：它表达的是"**玩家**这一帧按了
//! Shift + 决策键"，与具体单位无关（AI 不走这条路，它自己决定）。

use bevy::prelude::*;

use super::events::UseFocus;

/// Focus 上限。
pub const FOCUS_MAX: u32 = 3;
/// Focus 恢复间隔（虚拟秒）：世界在走才回，冻结时不回。
pub const FOCUS_RECOVER_INTERVAL: f32 = 10.0;

/// 反应资源（**挂在单位身上**）：**1 点 Focus = 把一次声明的前摇归零**。
///
/// 它买的是「反应速度」而不是数值：威胁压过来时，只有攒着 Focus 的人才来得及
/// 在同一瞬间改手（见 `docs/combat.md` 第五节）。
///
/// **每个单位各有一份**——玩家和敌人都会有，AI 因此能像玩家一样抢先手。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct Focus {
    pub current: u32,
    pub max: u32,
}

impl Default for Focus {
    fn default() -> Self {
        Self {
            current: FOCUS_MAX,
            max: FOCUS_MAX,
        }
    }
}

impl Focus {
    /// 还有没有余量。
    pub fn available(&self) -> bool {
        self.current > 0
    }

    /// 花掉 1 点；没有余量时返回 `false`（调用方据此退回普通前摇）。
    pub fn spend(&mut self) -> bool {
        if !self.available() {
            return false;
        }
        self.current -= 1;
        true
    }

    /// 回复 1 点（封顶）。
    pub fn recover(&mut self) {
        self.current = (self.current + 1).min(self.max);
    }
}

/// 本帧玩家有没有要求「用 Focus 换前摇归零」（`Shift` + 决策键）。
///
/// 由 [`track_pending_focus_system`](crate::timeline::track_pending_focus_system) 每帧写入，
/// 声明系统读它——真正的扣费发生在声明那一刻（没声明就不花）。
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct PendingFocus(pub bool);

impl PendingFocus {
    /// 本帧玩家要不要为这一手付 Focus。
    pub fn wants(&self) -> bool {
        self.0
    }
}

/// 玩家想不想用 Focus 换前摇（本帧有效）：`Shift` + 决策键 → [`UseFocus`]。
pub fn track_pending_focus_system(
    mut requests: MessageReader<UseFocus>,
    mut pending_focus: ResMut<PendingFocus>,
) {
    pending_focus.0 = requests.read().last().is_some();
}

/// Focus 回复：每 [`FOCUS_RECOVER_INTERVAL`] 虚拟秒回 1 点。
///
/// 走到 `Time<Virtual>` 上，因此**冻结时不回复**：暂停不是"白送资源"的时间。
///
/// **每个单位各有自己的计时**：`Timer` 挂在单位身上，所以新上场的单位不会
/// 蹭到别人的进度，也不会因为"全队共用一个计时器"而整齐地一起回。
#[derive(Component, Debug, Clone)]
pub struct FocusRecoverTimer(pub Timer);

impl Default for FocusRecoverTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(
            FOCUS_RECOVER_INTERVAL,
            TimerMode::Repeating,
        ))
    }
}

pub fn recover_focus_system(
    time: Res<Time<Virtual>>,
    mut focuses: Query<(&mut Focus, &mut FocusRecoverTimer)>,
) {
    for (mut focus, mut timer) in &mut focuses {
        if timer.0.tick(time.delta()).just_finished() {
            focus.recover();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::test_support::timeline_app;

    #[test]
    fn focus_spends_only_when_available_and_recovers_to_the_cap() {
        let mut focus = Focus::default();
        assert_eq!(focus.current, FOCUS_MAX);

        for _ in 0..FOCUS_MAX {
            assert!(focus.spend());
        }
        assert!(!focus.spend(), "没有余量时花不出去");
        assert_eq!(focus.current, 0);

        focus.recover();
        assert_eq!(focus.current, 1);
        for _ in 0..FOCUS_MAX {
            focus.recover();
        }
        assert_eq!(focus.current, FOCUS_MAX, "回复封顶");
    }

    /// Focus 只走虚拟时间：冻住时一分都不回。
    #[test]
    fn focus_recovers_only_while_the_world_runs() {
        let mut app = timeline_app();
        // Focus 现在挂在**单位**身上（每个单位一份），所以测试造一个单位
        let unit = app
            .world_mut()
            .spawn((
                Focus {
                    current: 0,
                    max: FOCUS_MAX,
                },
                FocusRecoverTimer::default(),
            ))
            .id();

        app.update();
        for _ in 0..30 {
            app.update();
        }
        assert_eq!(
            app.world().get::<Focus>(unit).unwrap().current,
            0,
            "冻结时不该回复 Focus"
        );

        app.world_mut().resource_mut::<Time<Virtual>>().unpause();
        for _ in 0..101 {
            app.update();
        }
        assert_eq!(
            app.world().get::<Focus>(unit).unwrap().current,
            1,
            "世界走了 10 秒就该回 1 点"
        );
    }

    /// 玩家想用 Focus 换前摇时，这个请求只在本帧有效。
    #[test]
    fn a_focus_request_lasts_exactly_one_frame() {
        let mut app = timeline_app();
        app.world_mut().write_message(UseFocus);
        app.update();
        assert!(app.world().resource::<PendingFocus>().wants());

        app.update();
        assert!(
            !app.world().resource::<PendingFocus>().wants(),
            "上一帧的请求不该延续到下一帧"
        );
    }
}
