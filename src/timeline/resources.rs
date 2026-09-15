//! 时间线资源：暂停原因集合、手动暂停开关、反应资源 `Focus`。
//!
//! 冻结的判据只有一条：**暂停原因集合非空**。谁想停世界就往集合里放一个原因，
//! 不想要了就撤掉——因此「玩家等输入」「手动暂停」「敌人正打过来」可以叠加，
//! 互不覆盖（这是旧的单门控系统最容易出错的地方）。

use std::collections::HashSet;

use bevy::prelude::*;

/// 暂停原因：手动暂停（空格）。
pub const MANUAL: &str = "manual";
/// 暂停原因：有一名 `InputDriven` 的行动者空着决策槽，正等玩家决策。
pub const SLOT_EMPTY: &str = "slot_empty";
/// 暂停原因：combat 检测到有威胁瞄准玩家（见 `combat::reaction`）。
pub const THREAT: &str = "threat";

/// 暂停原因集合：**整个游戏唯一的冻结判据**（`frozen ⟺ 非空`）。
#[derive(Resource, Debug, Default, Clone)]
pub struct PauseReasons(HashSet<String>);

impl PauseReasons {
    /// 现在冻着吗。
    pub fn is_frozen(&self) -> bool {
        !self.0.is_empty()
    }

    /// 某个原因在不在。
    pub fn contains(&self, reason: &str) -> bool {
        self.0.contains(reason)
    }

    /// 加上一个原因（已经在里面就什么也不做）。
    pub fn insert(&mut self, reason: String) {
        self.0.insert(reason);
    }

    /// 撤掉一个原因。
    pub fn remove(&mut self, reason: &str) {
        self.0.remove(reason);
    }

    /// 排序后的原因列表（HUD 展示用；排序让「同一批原因」永远显示成同一句话）。
    pub fn labels(&self) -> Vec<&str> {
        let mut labels: Vec<&str> = self.0.iter().map(String::as_str).collect();
        labels.sort_unstable();
        labels
    }
}

/// 手动暂停开关：`Space` 切换，随 [`PauseReasons`] 一起决定要不要停表。
///
/// 单独放一个资源（而不是直接读写 `Time<Virtual>`）是为了让**手动暂停能一直有效**：
/// 旧实现里「等玩家输入」的门控每帧都会 unpause 一次，空格因而只前进一帧。
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct ManualPause(pub bool);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_is_exactly_the_reason_set_being_non_empty() {
        let mut reasons = PauseReasons::default();
        assert!(!reasons.is_frozen(), "没有原因就不冻结");

        reasons.insert(MANUAL.to_string());
        reasons.insert(SLOT_EMPTY.to_string());
        assert!(reasons.is_frozen());
        assert_eq!(
            reasons.labels(),
            vec![MANUAL, SLOT_EMPTY],
            "多个原因可以叠加，互不覆盖"
        );

        reasons.remove(MANUAL);
        assert!(reasons.is_frozen(), "还有原因就仍然冻着");
        reasons.remove(SLOT_EMPTY);
        assert!(!reasons.is_frozen(), "原因撤空才解冻");
    }

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
}
