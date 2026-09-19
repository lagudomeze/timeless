//! ⚠️ **这一族不是时间线的概念**：`Focus` 是玩家的**游戏资源**（和 `combat::defense`
//! 的 `Stamina` 同类），按计划要搬去 `combat` 并改成挂在单位身上的组件。
//! 它现在寄住在这里，只是因为搬迁会和 `spawn` / HUD / 8 个声明点一起动，
//! 所以单独排了一步（见 TODO.md 的 M20）。
//!
//! 时间线里寄住的 Focus 一族：资源 [`Focus`] + 意图 [`FocusIntent`]
//! （暂停原因集合不在这里，见 [`clock`](super::clock)）。

use bevy::prelude::*;

use super::events::UseFocus;

/// Focus 上限。
pub const FOCUS_MAX: u32 = 3;
/// Focus 恢复间隔（虚拟秒）：世界在走才回，冻结时不回。
pub const FOCUS_RECOVER_INTERVAL: f32 = 10.0;

/// 反应资源：**1 点 Focus = 把一次声明的前摇归零**。
///
/// 它买的是「反应速度」而不是数值：威胁压过来时，只有攒着 Focus 的人才来得及
/// 在同一瞬间改手（见 [docs/timeline.md](../../../docs/timeline.md) 第六节）。
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
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
/// 由 [`track_focus_intent_system`](crate::timeline::track_focus_intent_system) 每帧写入，
/// 声明系统读它——真正的扣费发生在声明那一刻（没声明就不花）。
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct FocusIntent(pub bool);

/// 玩家想不想用 Focus 换前摇（本帧有效）：`Shift` + 决策键 → [`UseFocus`]。
pub fn track_focus_intent_system(
    mut requests: MessageReader<UseFocus>,
    mut intent: ResMut<FocusIntent>,
) {
    intent.0 = requests.read().last().is_some();
}

/// Focus 回复：每 [`FOCUS_RECOVER_INTERVAL`] 虚拟秒回 1 点。
///
/// 走到 `Time<Virtual>` 上，因此**冻结时不回复**：暂停不是"白送资源"的时间。
pub fn recover_focus_system(
    mut focus: ResMut<Focus>,
    time: Res<Time<Virtual>>,
    mut timer: Local<f32>,
) {
    *timer += time.delta_secs();
    while *timer >= FOCUS_RECOVER_INTERVAL {
        *timer -= FOCUS_RECOVER_INTERVAL;
        focus.recover();
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
        app.world_mut().resource_mut::<Focus>().current = 0;

        app.update();
        for _ in 0..30 {
            app.update();
        }
        assert_eq!(
            app.world().resource::<Focus>().current,
            0,
            "冻结时不该回复 Focus"
        );

        app.world_mut().resource_mut::<Time<Virtual>>().unpause();
        for _ in 0..101 {
            app.update();
        }
        assert_eq!(
            app.world().resource::<Focus>().current,
            1,
            "世界走了 10 秒就该回 1 点"
        );
    }

    /// 玩家想用 Focus 换前摇时，意图只在本帧有效。
    #[test]
    fn focus_intent_lasts_exactly_one_frame() {
        let mut app = timeline_app();
        app.world_mut().write_message(UseFocus);
        app.update();
        assert!(app.world().resource::<FocusIntent>().0);

        app.update();
        assert!(
            !app.world().resource::<FocusIntent>().0,
            "上一帧的意图不该延续到下一帧"
        );
    }
}
