//! 时间线状态资源：**唯一的暂停真相**。

use bevy::prelude::*;

/// 反应窗口：**敌人打过来时要不要停下来等玩家决定**。
///
/// 无回合模型默认已经会在「玩家就绪」时冻结时间；这个开关管的是**威胁**：
/// 场上有「正在前摇、且瞄准玩家」的攻击时，要不要也停。
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimelineConfig {
    pub reaction: ReactionWindow,
}

/// 反应窗口的三种松紧（按 `F2` 循环）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReactionWindow {
    /// 只要敌人有瞄准玩家的未结算攻击就冻结（**默认：最松，方便调试**）
    #[default]
    Loose,
    /// 只在玩家**能反应**（就绪）时冻结，且同一发攻击只停一次
    Strict,
    /// 完全不因威胁冻结
    Off,
}

impl ReactionWindow {
    /// `F2` 循环：Loose → Strict → Off → Loose。
    pub fn next(self) -> Self {
        match self {
            Self::Loose => Self::Strict,
            Self::Strict => Self::Off,
            Self::Off => Self::Loose,
        }
    }

    /// 状态行上显示的名字。
    pub fn label(self) -> &'static str {
        match self {
            Self::Loose => "loose",
            Self::Strict => "strict",
            Self::Off => "off",
        }
    }
}

/// 时间线状态：整个游戏唯一的暂停判据。
///
/// 无回合模型里不存在「阶段」：谁该决策由各单位自己的 `Ready` 决定
/// （见 [`crate::timeline::components::Ready`]），本资源只回答
/// 「现在要不要为玩家停下世界」。
#[derive(Resource, Debug, Default)]
pub struct Timeline {
    /// 玩家已就绪、世界正在等他做决定：此时冻结虚拟时间。
    waiting_for_input: bool,
    /// 本帧刚声明、还没被提交桥升为 `Pending` 的那条玩家行动。
    ///
    /// 提交桥每帧清一次，所以它的实际寿命只有一帧——HUD 靠它区分
    /// "正在等玩家决定"和"玩家这一手刚落地"。
    draft: Option<Entity>,
}

impl Timeline {
    /// 世界是否正在等玩家输入（HUD / 调试面板读它）。
    pub fn waiting_for_input(&self) -> bool {
        self.waiting_for_input
    }

    /// 玩家是否有"刚声明、还没进 `Pending`"的行动。
    pub fn has_draft(&self) -> bool {
        self.draft.is_some()
    }

    /// 每帧由 `timeline_gate_system` 写入。
    pub fn set_waiting_for_input(&mut self, waiting: bool) {
        self.waiting_for_input = waiting;
    }

    /// 记下 / 清掉玩家草案。
    pub fn set_draft(&mut self, draft: Option<Entity>) {
        self.draft = draft;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reaction_window_starts_loose_for_easy_debugging() {
        let config = TimelineConfig::default();
        assert!(
            matches!(config.reaction, ReactionWindow::Loose),
            "默认应当最松：敌人一动就停，方便调试"
        );
        assert_eq!(ReactionWindow::Loose.next(), ReactionWindow::Strict);
        assert_eq!(ReactionWindow::Strict.next(), ReactionWindow::Off);
        assert_eq!(ReactionWindow::Off.next(), ReactionWindow::Loose);
    }

    #[test]
    fn timeline_starts_not_waiting_and_without_a_draft() {
        let timeline = Timeline::default();
        assert!(!timeline.waiting_for_input());
        assert!(!timeline.has_draft());
    }

    #[test]
    fn waiting_flags_are_writable_by_the_gate() {
        let mut timeline = Timeline::default();
        timeline.set_waiting_for_input(true);
        timeline.set_draft(Some(Entity::PLACEHOLDER));
        assert!(timeline.waiting_for_input());
        assert!(timeline.has_draft());

        timeline.set_waiting_for_input(false);
        timeline.set_draft(None);
        assert!(!timeline.waiting_for_input());
        assert!(!timeline.has_draft());
    }
}
