//! # 战斗消息（Bevy 0.19 的 `Message`）
//!
//! 蓝图中用旧版 `#[derive(Event)]` / `add_event`，Bevy 0.19 已把缓冲式事件
//! 拆分为 `Message`（`MessageWriter` / `MessageReader`，双缓冲存活 2 帧），
//! 这里按 0.19 落地；定向即时响应才用 EntityEvent + Observer。
//!
//! - [`DamageEvent`]：伤害系统 → 扣血系统；
//! - [`DeathEvent`]：扣血系统 → 销毁系统。

use bevy::prelude::*;

/// 一次扣血请求（目标 + 扣除量）
#[derive(Message, Debug, Clone, Copy)]
pub struct DamageEvent {
    pub target: Entity,
    pub amount: f32,
}

/// 目标已死亡（生命 ≤ 0），请求销毁
#[derive(Message, Debug, Clone, Copy)]
pub struct DeathEvent {
    pub entity: Entity,
}
