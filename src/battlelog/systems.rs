//! 战斗日志：Message → 可读文本
use std::collections::VecDeque;

use bevy::prelude::*;

use crate::combat::Faction;
use crate::events::{DamageEvent, DeathEvent};

/// 战斗日志（环形保留最近 N 条）
#[derive(Resource, Debug)]
pub struct BattleLog {
    entries: VecDeque<String>,
    max: usize,
}

impl Default for BattleLog {
    fn default() -> Self {
        Self {
            entries: VecDeque::with_capacity(16),
            max: 16,
        }
    }
}

impl BattleLog {
    pub fn push(&mut self, message: impl Into<String>) {
        if self.entries.len() >= self.max {
            self.entries.pop_front();
        }
        self.entries.push_back(message.into());
    }

    pub fn entries(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(String::as_str)
    }
}

/// 把伤害 / 死亡消息转成阵营标签文本（供 console / UI）。
pub fn battle_log_system(
    mut damages: MessageReader<DamageEvent>,
    mut deaths: MessageReader<DeathEvent>,
    faction_q: Query<&Faction>,
    mut log: ResMut<BattleLog>,
) {
    for damage in damages.read() {
        let who = faction_q
            .get(damage.target)
            .map(faction_label)
            .unwrap_or("单位");
        let text = format!("{who} 受到 {:.0} 点伤害", damage.amount);
        log.push(&text);
        info!("[{text}]");
    }
    for death in deaths.read() {
        let who = faction_q
            .get(death.entity)
            .map(faction_label)
            .unwrap_or("单位");
        let text = format!("{who} 阵亡");
        log.push(&text);
        info!("[{text}]");
    }
}

fn faction_label(faction: &Faction) -> &'static str {
    match faction {
        Faction::Player => "玩家",
        Faction::Enemy => "敌人",
    }
}
