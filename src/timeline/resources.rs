//! 时间线状态资源：**唯一的暂停真相**。

use bevy::prelude::*;

/// 时间线配置。
///
/// `require_commit = false`（默认）= 输入直接生效；无回合模型里
/// 玩家不该为每个动作按两次键，因此默认值就是「按下即决定」。
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimelineConfig {
    /// `true` = 输入只产生草案，按 `Enter` 才提交；`false` = 输入直接生效。
    pub require_commit: bool,
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
    /// 玩家本轮尚未提交的草案（仅 `require_commit = true` 时有值）。
    draft: Option<Entity>,
}

impl Timeline {
    /// 世界是否正在等玩家输入（HUD / 调试面板读它）。
    pub fn waiting_for_input(&self) -> bool {
        self.waiting_for_input
    }

    /// 玩家是否有未提交的草案。
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
    fn default_config_executes_input_directly() {
        let config = TimelineConfig::default();
        assert!(
            !config.require_commit,
            "默认应当是「按下即决定」，而不是要求 Enter 确认"
        );
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
