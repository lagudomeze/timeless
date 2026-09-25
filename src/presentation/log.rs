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
/// **时刻戳来自消息本身**（`DamageEvent.at`，由写方在结算那一帧记下）：
/// 复盘要的正是"**哪个时刻**命中了谁"。日志不自己读时钟——那样写出来的是
/// "记录日志的那一刻"，差一帧，而且与死亡那一行对不上
/// （死亡继承致命一击的时刻，两行因此带同一个戳）。
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
        let who = side(damage.target).unwrap_or("单位");
        // **语序是「谁打谁、打多少」**：先说出手方、再说命中谁。
        // ⚠️ 这里曾经把受击方**直接拼在**出击方那句话前面（`format!("{who}{text}")`），
        // 而 `text` 本身又以出手方开头——于是同一句里两个阵营标签**贴在一起**，
        // 读出来是「敌人玩家 命中，受到 16 点伤害」（受击方在前、出手方在后，
        // 中间连空格都没有）。实机打一场就能看见，四条单测全都漏了它
        // （它们只断言"两个标签都出现"，不检查语序与分隔）。
        let text = match damage.source.and_then(side) {
            Some(attacker) => format!("{attacker} 命中 {who}，造成 {} 点伤害", damage.amount),
            None => format!("{who} 受到 {} 点伤害", damage.amount),
        };
        // 前缀 = 命中那一刻的虚拟时刻（复盘要的"什么时候"）
        let stamped = format!("[{:.1}s] {text}", damage.at);
        log.push(stamped.clone());
        info!("[{stamped}]");
    }
    for death in deaths.read() {
        let who = side(death.entity).unwrap_or("单位");
        let text = match death.killer.and_then(side) {
            Some(killer) => format!("{who} 被{killer}击杀"),
            None => format!("{who} 阵亡"),
        };
        let stamped = format!("[{:.1}s] {text}", death.at);
        log.push(stamped.clone());
        info!("[{stamped}]");
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
    ///
    /// ⚠️ **判据是整句，不只是"两个标签都出现"**：这条以前只断言 `contains("玩家")`
    /// 与 `contains("敌人")`，于是漏掉了一个真实的显示 bug——那一版把受击方**直接
    /// 拼在**出击方那句话前面，读出来是「敌人玩家 命中，受到 16 点伤害」
    /// （两个标签贴在一起、语序还反了）。**两个标签都在**，所以旧断言全绿。
    /// 现在钉住**确切的那句**：语序（出手方在前）与分隔（`命中 {who}` 之间有空格）。
    #[test]
    fn a_hit_names_the_side_that_struck() {
        let (mut app, player, attack) = log_app();
        app.world_mut().write_message(DamageEvent {
            source: Some(attack),
            target: player,
            amount: 12,
            at: 3.5,
        });
        app.update();

        assert_eq!(
            last(&app),
            "[3.5s] 敌人 命中 玩家，造成 12 点伤害",
            "前缀是命中时刻，语序 = 出手方 → 受击方 → 数值"
        );
    }

    /// **哪个时刻命中了谁**：日志行的前缀取自消息自带的虚拟时刻，
    /// 而不是"记录日志的那一刻"含糊过去。
    #[test]
    fn a_line_is_stamped_with_the_moment_it_happened() {
        let (mut app, player, attack) = log_app();
        app.world_mut().write_message(DamageEvent {
            source: Some(attack),
            target: player,
            amount: 7,
            at: 12.3,
        });
        app.update();

        assert!(
            last(&app).starts_with("[12.3s]"),
            "命中行要带那一刻的虚拟时刻，实际：{}",
            last(&app)
        );
    }

    /// 受击方与出手方**不能贴在一起**（那种句子读不出是谁打谁）。
    ///
    /// 单独一条守着这个形状：两方标签相邻时（"敌人玩家"）必然是漏了分隔。
    #[test]
    fn the_two_side_labels_are_never_glued_together() {
        let (mut app, player, attack) = log_app();
        app.world_mut().write_message(DamageEvent {
            source: Some(attack),
            target: player,
            amount: 1,
            at: 0.0,
        });
        app.update();

        let text = last(&app);
        for glued in ["敌人玩家", "玩家敌人"] {
            assert!(
                !text.contains(glued),
                "两方标签贴在一起了（{glued}）：{text}"
            );
        }
    }

    /// 环境伤害（`source: None`）没有出手方，照旧只写受击方——不能 panic 也不能写"被未知"。
    #[test]
    fn environmental_damage_has_no_attacker() {
        let (mut app, player, _) = log_app();
        app.world_mut().write_message(DamageEvent {
            source: None,
            target: player,
            amount: 3,
            at: 0.0,
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
            at: 8.0,
        });
        app.update();

        let text = last(&app);
        assert_eq!(text, "[8.0s] 玩家 被敌人击杀", "死亡行也要带致命一击的时刻");
    }

    /// 没有击杀者（环境致死）时退回"阵亡"，不写"被未知击杀"。
    #[test]
    fn a_death_without_a_killer_stays_plain() {
        let (mut app, player, _) = log_app();
        app.world_mut().write_message(DeathEvent {
            entity: player,
            killer: None,
            at: 0.0,
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
