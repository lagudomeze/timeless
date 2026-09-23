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
///
/// **写出「谁打的谁」**（死亡复盘的底子，见 `docs/game-design.md`「信息即力量」）：
/// 伤害的来源 `DamageEvent.source` 是**攻击实体**（箭 / 横扫 / 火球），
/// 它身上挂着投掷方的 `Faction` 而不是 `Health`，所以这里读的是**出手方的阵营**
/// ——"敌人打了你"比"某个实体打了你"更像玩家想看的那句话。
/// 环境伤害（`source: None`）不带主，照旧只写受击方。
///
/// ⚠️ **时间戳还没有**：复盘要的"**哪个时刻**命中了谁"需要虚拟时间进入
/// `DamageEvent`，而它现在不带时间。本轮只做到"谁打谁"。
pub fn battle_log_system(
    mut damages: MessageReader<DamageEvent>,
    mut deaths: MessageReader<DeathEvent>,
    faction_q: Query<&Faction>,
    mut log: ResMut<BattleLog>,
) {
    // 攻击实体（来源）与单位（目标）都在这个查询里——两者都带 `Faction`
    let side = |entity: Entity| faction_q.get(entity).map(faction_label).ok();
    for damage in damages.read() {
        // 伤害类型不再是一个中心枚举：每种伤害有各自的组件与系统，
        // 日志因此只说"扣了多少"（要写类型就在各自的系统里补一条消息）
        let text = match damage.source.and_then(side) {
            Some(attacker) => format!("{attacker} 命中，受到 {} 点伤害", damage.amount),
            None => format!("受到 {} 点伤害", damage.amount),
        };
        let who = side(damage.target).unwrap_or("单位");
        let text = format!("{who}{text}");
        log.push(&text);
        info!("[{text}]");
    }
    for death in deaths.read() {
        let who = side(death.entity).unwrap_or("单位");
        let text = match death.killer.and_then(side) {
            Some(killer) => format!("{who} 被{killer}击杀"),
            None => format!("{who} 阵亡"),
        };
        log.push(&text);
        info!("[{text}]");
    }
}

/// 阵营标签（HUD 日志用中文）。
///
/// `pub` 是为了让表现层的其它地方有一处统一叫法，不在多处各写一份"玩家 / 敌人"。
pub fn faction_label(faction: &Faction) -> &'static str {
    match faction {
        Faction::Player => "玩家",
        Faction::Enemy => "敌人",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 攻击实体带 `Faction` 但不带 `Health`——日志靠这一点认出"谁出手的"。
    fn log_app() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<BattleLog>()
            .add_message::<DamageEvent>()
            .add_message::<DeathEvent>()
            .add_systems(Update, battle_log_system);
        let player = app.world_mut().spawn(Faction::Player).id();
        let attack = app.world_mut().spawn(Faction::Enemy).id(); // 攻击实体：只有阵营
        (app, player, attack)
    }

    fn last(app: &App) -> String {
        app.world()
            .resource::<BattleLog>()
            .entries()
            .last()
            .unwrap_or_default()
            .to_string()
    }

    /// **写得出"谁打的谁"**：伤害的来源是攻击实体，日志读它的阵营来认出手方。
    #[test]
    fn a_hit_names_the_side_that_struck() {
        let (mut app, player, attack) = log_app();
        app.world_mut().write_message(DamageEvent {
            source: Some(attack),
            target: player,
            amount: 12,
        });
        app.update();

        let text = last(&app);
        assert!(text.contains("玩家"), "要点出被打的是谁：{text}");
        assert!(text.contains("敌人"), "要点出是谁打的：{text}");
        assert!(text.contains("12"), "伤害数值不能丢：{text}");
    }

    /// 环境伤害（`source: None`）没有出手方，照旧只写受击方——不能 panic 也不能写"被未知"。
    #[test]
    fn environmental_damage_has_no_attacker() {
        let (mut app, player, _) = log_app();
        app.world_mut().write_message(DamageEvent {
            source: None,
            target: player,
            amount: 3,
        });
        app.update();

        let text = last(&app);
        assert!(text.contains("玩家"), "{text}");
        assert!(
            !text.contains("敌人"),
            "没有来源时不该凭空写一个出手方：{text}"
        );
    }

    /// **死亡复盘**：阵亡那一行要说清是被谁击杀的（`DeathEvent.killer` 一直被忽略）。
    #[test]
    fn a_death_names_the_killer() {
        let (mut app, player, attack) = log_app();
        app.world_mut().write_message(DeathEvent {
            entity: player,
            killer: Some(attack),
        });
        app.update();

        let text = last(&app);
        assert!(text.contains("玩家"), "{text}");
        assert!(text.contains("敌人"), "要点出是谁击杀的：{text}");
    }

    /// 没有击杀者（环境致死）时退回"阵亡"，不写"被未知击杀"。
    #[test]
    fn a_death_without_a_killer_stays_plain() {
        let (mut app, player, _) = log_app();
        app.world_mut().write_message(DeathEvent {
            entity: player,
            killer: None,
        });
        app.update();
        assert!(last(&app).contains("阵亡"));
    }

    /// 环形缓冲：超过上限丢最旧的，留下最近的。
    #[test]
    fn the_log_keeps_only_the_most_recent_entries() {
        let mut log = BattleLog::default();
        for i in 0..20 {
            log.push(format!("第 {i} 条"));
        }
        let entries: Vec<&str> = log.entries().collect();
        assert_eq!(entries.len(), 16, "上限是 16 条");
        assert_eq!(entries.last(), Some(&"第 19 条"), "留下的是最近的");
        assert_eq!(entries.first(), Some(&"第 4 条"), "最旧的被丢掉");
    }
}
