//! 时钟域插件：注册暂停设施，接线唯一的时钟写入系统。

use bevy::prelude::*;

use super::{ClockSet, ManualPause, PauseReasons, PauseRequest, process_pause_requests};

/// 世界冻结设施（通用，不认识任何领域）。
#[derive(Debug, Default)]
pub struct ClockPlugin;

impl Plugin for ClockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PauseReasons>()
            // 玩家的手动暂停：一个布尔（原因集合只装「别人为什么在停表」）
            .init_resource::<ManualPause>()
            // 暂停请求：写方是各领域（断言）与输入域（Toggle），消费方是 `process_pause_requests`
            .add_message::<PauseRequest>()
            .add_systems(Update, process_pause_requests.in_set(ClockSet));
    }
}
