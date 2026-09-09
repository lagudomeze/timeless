//! 血量组件

use bevy::prelude::*;

/// 当前 / 最大生命值（f32，便于渐进扣减）
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Default for Health {
    fn default() -> Self {
        Self::new(100.0)
    }
}

impl Health {
    pub fn new(max: f32) -> Self {
        Self { current: max, max }
    }

    pub fn is_alive(&self) -> bool {
        self.current > 0.0
    }
}

// 用于向健康系统发送的纯净事件
#[derive(Message, Debug, Clone, Copy)]
pub struct ModifyHealthEvent {
    pub target: Entity,
    pub amount: f32, // 负数为扣血，正数为治疗
}

/// 目标已死亡（生命 ≤ 0），请求销毁
#[derive(Message, Debug, Clone, Copy)]
pub struct DeathEvent {
    pub entity: Entity,
}

/// 扣血：对每条 `DamageEvent` 扣减目标 `Health`；生命归零时发出 `DeathEvent`。
/// 目标已死或已不存在时跳过，避免重复死亡消息。
pub fn apply_damage(
    mut modify_events: MessageReader<ModifyHealthEvent>,
    mut deaths: MessageWriter<DeathEvent>,
    mut health_q: Query<&mut Health>,
) {
    for event in modify_events.read() {
        let Ok(mut health) = health_q.get_mut(event.target) else {
            continue; // 目标已不存在
        };
        if !health.is_alive() {
            continue;
        }
        health.current = (health.current + event.amount).max(0.0);
        if !health.is_alive() {
            info!("☠ {:?} 生命归零，发出 DeathEvent", event.target);
            deaths.write(DeathEvent {
                entity: event.target,
            });
        }
    }
}
