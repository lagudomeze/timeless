//! 战斗日志：把战斗消息翻译成可读文本，供 HUD / 复盘读取。

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::combat::{DamageEvent, DeathEvent, Faction};

/// 战斗日志（环形保留最近 N 条）。
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
    /// 追加一条（超出上限时丢弃最旧的）。
    pub fn push(&mut self, message: impl Into<String>) {
        if self.entries.len() >= self.max {
            self.entries.pop_front();
        }
        self.entries.push_back(message.into());
    }

    /// 按时间顺序读取日志。
    pub fn entries(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(String::as_str)
    }
}

/// 消费伤害 / 死亡消息，写入可读文本（当前同时输出到控制台）。
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
        let text = format!(
            "{who} 受到 {:.0} 点{}伤害",
            damage.amount,
            damage.kind.label()
        );
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
