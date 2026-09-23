//! 单位面板的**模型**：一块面板要显示的那几个数，以及把它们变成文案 / 条宽的纯函数。
//!
//! 这里没有系统、没有实体、没有 `Node`——输入是普通的 `UnitRow`，输出是 `String`
//! 与 `Val::Percent`。因此"状态行该写什么、条该多宽"可以脱离 App 直接单测。
//!
//! 「滑动条」是**只读进度条**：条长 = 数值比例，本层不改任何游戏状态。

use bevy::prelude::*;

use crate::ai::Tactic;
use crate::combat::defense::Stamina;
use crate::combat::{Faction, Health};
use crate::movement::Cell;
use crate::timeline::DecisionSlot;

/// 数值 → 0..=1 的比例（`max <= 0` 视为空）。
pub fn bar_fraction(current: f32, max: f32) -> f32 {
    if max <= 0.0 {
        0.0
    } else {
        (current / max).clamp(0.0, 1.0)
    }
}

/// 数值 → 条宽（`Val::Percent`）。
pub fn bar_percent(current: f32, max: f32) -> Val {
    Val::Percent(bar_fraction(current, max) * 100.0)
}

/// 一帧的单位状态（面板文本全部由它推导）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnitRow {
    pub faction: Faction,
    pub health: Health,
    pub stamina: Option<Stamina>,
    pub cell: Cell,
    pub position: Vec3,
    /// 决策槽：`Empty` = 现在能决策（面板显示 `ready`）
    pub slot: DecisionSlot,
    pub dodging: bool,
    pub parrying: bool,
    pub airborne: bool,
    pub tactic: Option<Tactic>,
}

/// 面板快照缓存：与上一帧完全相同就整帧不碰 UI。
#[derive(Debug, Default, Clone, PartialEq)]
pub struct UnitPanelCache {
    pub rows: [Option<UnitRow>; 2],
}

/// 阵营 → 快照下标（玩家在前）。
pub fn slot(faction: Faction) -> usize {
    match faction {
        Faction::Player => 0,
        Faction::Enemy => 1,
    }
}

/// 两个敌人之间，哪个更该显示在面板上。
///
/// 规则：**离玩家更近的优先**；一样近时取格坐标更小的（`Cell` 的序不影响数值，
/// 只用来把并列打散，保证结果与遍历顺序无关）。没有玩家可参照时按格坐标。
fn enemy_rank(candidate: &UnitRow, current: &UnitRow, player: Option<&UnitRow>) -> bool {
    let by_player = |row: &UnitRow| {
        player
            .map(|player| row.position.distance(player.position))
            .unwrap_or(f32::INFINITY)
    };
    let (candidate_distance, current_distance) = (by_player(candidate), by_player(current));
    if candidate_distance != current_distance {
        return candidate_distance < current_distance;
    }
    (candidate.cell.x, candidate.cell.z) < (current.cell.x, current.cell.z)
}

/// 一帧的面板快照（两条面板各自的读数）。
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct UnitPanels {
    pub rows: [Option<UnitRow>; 2],
}

impl UnitPanels {
    /// 按阵营把读数摆进两个槽位。
    ///
    /// **玩家只有一个**；敌人可能有很多，而面板只画得下一个——所以定一条明确的规则：
    /// **显示离玩家最近的那个敌人**，距离相同时取格坐标小的那个。
    /// 判据完全由数据决定（不含遍历顺序），所以同一份战场状态永远得到同一个面板。
    ///
    /// 之前这里是"后遍历到的覆盖前面"，而 **ECS 查询顺序不保证**——
    /// 两个敌人时显示谁全凭运气，看上去像血条自己在跳。
    pub fn from_rows(rows: &[UnitRow]) -> Self {
        let mut snapshot: [Option<UnitRow>; 2] = [None, None];
        let player = rows.iter().find(|row| row.faction == Faction::Player);
        for row in rows {
            let index = slot(row.faction);
            if row.faction == Faction::Player {
                snapshot[index] = Some(*row);
                continue;
            }
            let wins = match snapshot[index] {
                None => true,
                Some(current) => enemy_rank(row, &current, player),
            };
            if wins {
                snapshot[index] = Some(*row);
            }
        }
        Self { rows: snapshot }
    }
    /// 玩家位置（敌人面板要拿它算距离）。
    pub fn player_position(&self) -> Option<Vec3> {
        self.of(Faction::Player).map(|row| row.position)
    }

    /// 取某一方的读数。
    pub fn of(&self, faction: Faction) -> Option<&UnitRow> {
        self.rows[slot(faction)].as_ref()
    }

    /// HP 条宽。
    pub fn hp_percent(&self, faction: Faction) -> Val {
        self.of(faction)
            .map(|row| bar_percent(row.health.current as f32, row.health.max as f32))
            .unwrap_or(Val::Percent(0.0))
    }

    /// 精力条宽（没有精力组件的单位显示空条）。
    pub fn stamina_percent(&self, faction: Faction) -> Val {
        self.of(faction)
            .and_then(|row| row.stamina)
            .map(|stamina| bar_percent(stamina.current as f32, stamina.max as f32))
            .unwrap_or(Val::Percent(0.0))
    }

    /// HP 文本。
    pub fn hp_text(&self, faction: Faction) -> String {
        self.of(faction)
            .map(|row| format!("HP {} / {}", row.health.current, row.health.max))
            .unwrap_or_default()
    }

    /// 精力文本。
    pub fn stamina_text(&self, faction: Faction) -> String {
        self.of(faction)
            .map(|row| match row.stamina {
                Some(stamina) => format!("EN {} / {}", stamina.current, stamina.max),
                None => "EN -".to_string(),
            })
            .unwrap_or_default()
    }

    /// 状态行文本（只有敌人行会带上到玩家的距离）。
    pub fn state_line(&self, faction: Faction) -> String {
        let Some(row) = self.of(faction) else {
            return String::new();
        };
        let distance = (faction == Faction::Enemy)
            .then(|| {
                self.player_position()
                    .map(|player| row.position.distance(player))
            })
            .flatten();
        row.state_line(distance)
    }
}

impl UnitRow {
    /// 状态行：`PLAYER · ready · cell (1,1)`。
    pub fn state_line(&self, to_player: Option<f32>) -> String {
        let name = match self.faction {
            Faction::Player => "PLAYER",
            Faction::Enemy => "ENEMY",
        };
        // 跳跃是「谁都别想插队」的状态，值得单独标出来
        let defense = if self.airborne {
            format!("{} · air", self.defense_label())
        } else {
            self.defense_label().to_string()
        };
        let mut line = format!(
            "{name} · {defense} · cell ({:>2},{:>2})",
            self.cell.x, self.cell.z
        );
        if let Some(distance) = to_player {
            line.push_str(&format!(" · dist {distance:.1}"));
        }
        if let Some(tactic) = self.tactic {
            line.push_str(&format!(" · {}", tactic_label(tactic)));
        }
        line
    }

    /// 防御 / 就绪状态：防御标记优先。
    fn defense_label(&self) -> &'static str {
        if self.dodging {
            "dodging"
        } else if self.parrying {
            "parrying"
        } else if self.slot.ready() {
            "ready"
        } else {
            "busy"
        }
    }
}

/// 敌人战术的可读标签。
pub fn tactic_label(tactic: Tactic) -> &'static str {
    match tactic {
        Tactic::Idle => "idle",
        Tactic::Approach => "approach",
        Tactic::Melee => "melee",
        Tactic::Shoot => "shoot",
        Tactic::Retreat => "retreat",
        Tactic::Dodge => "dodge",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::decision::BUSY_SENTINEL;

    #[test]
    fn bar_percent_maps_the_ratio_and_clamps() {
        assert_eq!(bar_percent(50.0, 100.0), Val::Percent(50.0));
        assert_eq!(bar_percent(5.0, 5.0), Val::Percent(100.0));
        assert_eq!(bar_percent(-3.0, 10.0), Val::Percent(0.0), "负值夹到 0");
        assert_eq!(bar_percent(30.0, 10.0), Val::Percent(100.0), "溢出夹到 100");
        assert_eq!(bar_percent(1.0, 0.0), Val::Percent(0.0), "max = 0 视为空");
    }

    fn row(faction: Faction, cell: Cell, position: Vec3) -> UnitRow {
        UnitRow {
            faction,
            health: Health::new(50),
            stamina: Some(Stamina::new(3)),
            cell,
            position,
            slot: DecisionSlot::Idle { intent: None },
            dodging: false,
            parrying: false,
            airborne: false,
            tactic: None,
        }
    }

    #[test]
    fn state_line_carries_defense_and_intent() {
        let mut enemy = row(Faction::Enemy, Cell::new(3, 3), Vec3::ZERO);
        enemy.slot = DecisionSlot::Executing {
            until: BUSY_SENTINEL,
        };
        enemy.dodging = true;
        enemy.tactic = Some(Tactic::Approach);

        let line = enemy.state_line(Some(7.12));
        assert!(line.contains("ENEMY"), "{line}");
        assert!(
            line.contains("dodging"),
            "防御标记优先于 ready/busy：{line}"
        );
        assert!(line.contains("cell ( 3, 3)"), "{line}");
        assert!(line.contains("dist 7.1"), "{line}");
        assert!(line.contains("approach"), "{line}");
    }

    /// 只有敌人那一行带距离——玩家面板不需要"离自己多远"。
    ///
    /// 这条守着一个曾经的写法：距离在写 UI 的循环里临时算，于是
    /// `state_line` 的调用方决定了读数的内容，模型层测不出来。
    #[test]
    fn only_the_enemy_line_carries_the_distance_to_the_player() {
        let panels = UnitPanels::from_rows(&[
            row(Faction::Player, Cell::new(0, 0), Vec3::ZERO),
            row(Faction::Enemy, Cell::new(3, 0), Vec3::new(6.0, 0.0, 0.0)),
        ]);

        let player_line = panels.state_line(Faction::Player);
        let enemy_line = panels.state_line(Faction::Enemy);
        assert!(!player_line.contains("dist"), "{player_line}");
        assert!(enemy_line.contains("dist 6.0"), "{enemy_line}");
    }

    /// 某一方不在场时读数退回空值，而不是上一帧的残留。
    #[test]
    fn a_missing_faction_reads_as_empty() {
        let panels = UnitPanels::from_rows(&[row(Faction::Player, Cell::new(0, 0), Vec3::ZERO)]);

        assert_eq!(panels.state_line(Faction::Enemy), "");
        assert_eq!(panels.hp_text(Faction::Enemy), "");
        assert_eq!(panels.stamina_text(Faction::Enemy), "");
        assert_eq!(panels.hp_percent(Faction::Enemy), Val::Percent(0.0));
        assert_eq!(panels.stamina_percent(Faction::Enemy), Val::Percent(0.0));
    }

    /// 没有精力组件的单位（例如场景里的纯装饰靶子）显示 `EN -` 而不是 0/0。
    #[test]
    fn a_unit_without_stamina_shows_a_dash() {
        let mut without = row(Faction::Player, Cell::new(0, 0), Vec3::ZERO);
        without.stamina = None;
        let panels = UnitPanels::from_rows(&[without]);

        assert_eq!(panels.stamina_text(Faction::Player), "EN -");
        assert_eq!(panels.stamina_percent(Faction::Player), Val::Percent(0.0));
    }

    /// 两个敌人时面板显示**离玩家最近的那个**，与遍历顺序无关。
    ///
    /// 这条守着一个真实的症状：以前是"后遍历到的覆盖前面"，而 ECS 查询顺序不保证，
    /// 于是两个敌人时血条看起来自己在跳。
    #[test]
    fn the_enemy_panel_shows_the_nearest_one_whatever_the_iteration_order() {
        let player = row(Faction::Player, Cell::new(0, 0), Vec3::ZERO);
        let near = row(Faction::Enemy, Cell::new(1, 0), Vec3::new(2.0, 0.0, 0.0));
        let far = row(Faction::Enemy, Cell::new(5, 0), Vec3::new(10.0, 0.0, 0.0));

        for order in [
            vec![player, near, far],
            vec![player, far, near],
            vec![far, near, player],
        ] {
            let panels = UnitPanels::from_rows(&order);
            let shown = panels.of(Faction::Enemy).expect("应当有敌人在面板上");
            assert_eq!(
                shown.cell,
                Cell::new(1, 0),
                "无论遍历顺序如何，显示的都该是更近的那个"
            );
        }
    }

    /// 一样近时按格坐标打散——判据完全由数据决定，不含遍历顺序。
    #[test]
    fn equidistant_enemies_are_broken_by_cell_order_not_iteration() {
        let player = row(Faction::Player, Cell::new(0, 0), Vec3::ZERO);
        let east = row(Faction::Enemy, Cell::new(2, 0), Vec3::new(4.0, 0.0, 0.0));
        let west = row(Faction::Enemy, Cell::new(-2, 0), Vec3::new(-4.0, 0.0, 0.0));

        let a = UnitPanels::from_rows(&[player, east, west]);
        let b = UnitPanels::from_rows(&[player, west, east]);
        assert_eq!(
            a.of(Faction::Enemy).map(|row| row.cell),
            b.of(Faction::Enemy).map(|row| row.cell),
            "并列时两种顺序必须给出同一个答案"
        );
    }

    /// 单个敌人时照旧显示他（多敌人的改动不能影响只有一个的情况）。
    #[test]
    fn a_single_enemy_is_still_shown() {
        let panels = UnitPanels::from_rows(&[
            row(Faction::Player, Cell::new(0, 0), Vec3::ZERO),
            row(Faction::Enemy, Cell::new(3, 0), Vec3::new(6.0, 0.0, 0.0)),
        ]);
        assert_eq!(
            panels.of(Faction::Enemy).map(|row| row.cell),
            Some(Cell::new(3, 0))
        );
    }
}
