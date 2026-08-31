//! # 动作提示：移动方向箭头
//!
//! 玩家排定移动速度时，用 Bevy 内置 Gizmos 绘制黑色方向箭头
//! （即时模式：每帧按 Declared `MoveTo` 动作决定是否绘制，动作清除后下一帧自动消失，
//! 无需实体生命周期管理）。

use bevy::prelude::*;

use crate::combat::Player;
use crate::display::map::{cell_x, cell_z};
use crate::movement::{MoveTo, Position};
use crate::timeline::{Declared, ScheduledAction};

/// 移动方向箭头：玩家声明 `MoveTo` 时绘制黑色箭头（起点 = 当前格中心，
/// 终点 = 当前格 + 速度，箭头尖指向目标格中心）；动作移除后自动消失。
pub fn move_arrow_system(
    mut gizmos: Gizmos,
    player_q: Query<(Entity, &Position), With<Player>>,
    moves: Query<(&ScheduledAction, &MoveTo), With<Declared>>,
) {
    let Ok((player, pos)) = player_q.single() else {
        return;
    };
    let Some((_, mov)) = moves.iter().find(|(s, _)| s.actor == player) else {
        return;
    };
    let to = pos.0 + mov.velocity;
    // 略高于地面（y=0.02），配合 depth_bias 避免与地砖 z-fighting
    let from_w = Vec3::new(cell_x(pos.0.x), 0.02, cell_z(pos.0.y));
    let to_w = Vec3::new(cell_x(to.x), 0.02, cell_z(to.y));
    gizmos
        .arrow(from_w, to_w, Color::BLACK)
        .with_tip_length(0.3);
}

/// Gizmos 全局配置（默认组）：贴地箭头略前移避免 z-fighting，线宽调粗便于辨认。
/// 后续其他调试覆盖（火球轨迹、AI 目标线等）共用此配置。
pub fn configure_gizmos(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
    config.depth_bias = -0.01;
    config.line.width = 3.0;
}
