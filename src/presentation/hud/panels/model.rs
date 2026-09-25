//! 单位面板的**模型**：一块面板要显示的那几个数，以及把它们变成文案 / 条宽的纯函数。
//!
//! 这里没有系统、没有实体、没有 `Node`——输入是普通的 `UnitRow`，输出是 `String`
//! 与 `Val::Percent`。因此"状态行该写什么、条该多宽"可以脱离 App 直接单测。
//!
//! 「滑动条」是**只读进度条**：条长 = 数值比例，本层不改任何游戏状态。

use bevy::prelude::*;

use crate::ai::Tactic;
use crate::combat::Ammo;
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
    /// 弹药（远程 / 重击那条线）：`None` = 这个单位没有弹药组件
    pub ammo: Option<Ammo>,
    pub cell: Cell,
    pub position: Vec3,
    /// 决策槽：`Empty` = 现在能决策（面板显示 `ready`）
    pub slot: DecisionSlot,
    pub dodging: bool,
    pub parrying: bool,
    pub airborne: bool,
    pub tactic: Option<Tactic>,
    /// 有效护甲（基础 + 装备加成）；`None` = 这个单位没有护甲组件。
    ///
    /// 取数时**已经算好**（由 `equipment::armor_of` 合成）：面板只显示一个数，
    /// 不必知道"基础 / 加成"的结构。装备改动因此在这个读数上直接看得见。
    pub armor: Option<i32>,
    /// **洞察力读数**（`docs/insight.md` 第四节）：敌人此刻"会什么、够多远、
    /// 这一手多难打断"。`None` = 这个单位没有可读的能力（玩家面板不显示）。
    ///
    /// 它把"信息即力量"落成看得见的数：射程决定"我站哪儿安全"，
    /// 打断抗性决定"该躲还是该抢一手打掉它"。
    pub insight: Option<Insight>,
}

/// 一个敌人的**洞察力读数**（纯数据，由 `insight_of` 算出来）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Insight {
    /// 射程（格）：它的攻击够得到多远
    pub range_cells: u32,
    /// 正在前摇的那一手的打断抗性（`None` = 此刻没有前摇中的行动）
    pub interrupt_resist: Option<i32>,
    /// 它当前在用什么战术（读 `Tactic`，与"会什么技能"是同一层信息）
    pub tactic: Option<Tactic>,
}

/// 一帧的洞察力读数（**纯函数**：只吃已经摘好的事实，可脱离 App 单测）。
///
/// `range_cells` 由调用方从 `AttackRange` 取（那一层的世界里它就是格）；
/// `interrupt_resist` 从**正在前摇的那一条行动**上取——没有前摇就没有读数
/// （"它现在这一手能不能打断"只在有那一手时才有意义）。
pub fn insight_of(
    range_cells: Option<u32>,
    interrupt_resist: Option<i32>,
    tactic: Option<Tactic>,
) -> Option<Insight> {
    // 三个读数全无（没有射程、没有前摇、没有战术）就没什么可说的
    if range_cells.is_none() && interrupt_resist.is_none() && tactic.is_none() {
        return None;
    }
    Some(Insight {
        range_cells: range_cells.unwrap_or(0),
        interrupt_resist,
        tactic,
    })
}

/// 面板快照缓存：与上一帧完全相同就整帧不碰 UI。
#[derive(Debug, Default, Clone, PartialEq)]
pub struct UnitPanelCache {
    pub player: Option<UnitRow>,
    pub enemies: Vec<UnitRow>,
}

/// 面板上的一格：**玩家独占一格，敌人各占一格**。
///
/// 敌人那格带**名次**（按"离玩家最近"排序），因为面板要画的不再是"那一方"，
/// 而是"第几个敌人"——这正是"多敌人面板"要解决的问题。
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelSlot {
    Player,
    /// 第 `index` 个敌人（`0` = 离玩家最近的那个）
    Enemy(usize),
}

impl PanelSlot {
    /// 这一格属于哪个阵营（颜色与"要不要显示距离"都看它）。
    pub fn faction(self) -> Faction {
        match self {
            Self::Player => Faction::Player,
            Self::Enemy(_) => Faction::Enemy,
        }
    }
}

/// 敌人面板最多画几行。
///
/// **一处真相**：场景按它建行池、模型按它截断，两边不会分叉
/// （有测试钉住"行池大小 = 这个常量"）。
pub const MAX_ENEMY_ROWS: usize = 3;

impl Insight {
    /// 一行洞察力读数：`range 1 · break 3 · approach`。
    ///
    /// **每个数都回答一个具体问题**（`docs/insight.md` 第五节）：
    /// 射程 → "我站哪儿安全"；打断抗性 → "该躲还是该抢一手打掉它"；
    /// 战术 → "它想干什么"。没有的那几项**整段不出现**（不留空段）。
    pub fn line(&self) -> String {
        let mut parts = vec![format!("range {}", self.range_cells)];
        if let Some(resist) = self.interrupt_resist {
            parts.push(format!("break {resist}"));
        }
        if let Some(tactic) = self.tactic {
            parts.push(tactic_label(tactic).to_string());
        }
        parts.join(" · ")
    }
}

/// 敌人之间的顺序：**离玩家更近的在前**；一样近时取格坐标更小的。
///
/// `Cell` 的序不影响任何数值，只用来把并列打散——保证结果**与遍历顺序无关**。
/// 没有玩家可参照时（还没组装出来）全按格坐标。
fn enemy_order(a: &UnitRow, b: &UnitRow, player: Option<&UnitRow>) -> std::cmp::Ordering {
    let by_player = |row: &UnitRow| {
        player
            .map(|player| row.position.distance(player.position))
            .unwrap_or(f32::INFINITY)
    };
    by_player(a)
        .total_cmp(&by_player(b))
        .then_with(|| (a.cell.x, a.cell.z).cmp(&(b.cell.x, b.cell.z)))
}

/// 一帧的面板快照：玩家一格 + **敌人 N 格**。
#[derive(Debug, Default, Clone, PartialEq)]
pub struct UnitPanels {
    pub player: Option<UnitRow>,
    /// 按"离玩家最近"排好序的敌人，最多 [`MAX_ENEMY_ROWS`] 个
    pub enemies: Vec<UnitRow>,
}

impl UnitPanels {
    /// 把这一帧的单位读数整理成面板要画的那几格。
    ///
    /// 规则只有一条：**敌人按"离玩家最近"排序**，取前 [`MAX_ENEMY_ROWS`] 个。
    /// 判据完全由数据决定（不含遍历顺序），所以同一份战场状态永远得到同一个面板
    /// ——之前这里是"后遍历到的覆盖前面"，而 **ECS 查询顺序不保证**，
    /// 两个敌人时显示谁全凭运气，看上去像血条自己在跳。
    ///
    /// 排序规则与旧的"两个敌人取更近的那个"**完全一致**，只是从"挑一个"变成
    /// "排全部"——所以只出一个敌人时，显示谁这件事没有变化。
    pub fn from_rows(rows: &[UnitRow]) -> Self {
        let player = rows
            .iter()
            .find(|row| row.faction == Faction::Player)
            .copied();
        let mut enemies: Vec<UnitRow> = rows
            .iter()
            .filter(|row| row.faction == Faction::Enemy)
            .copied()
            .collect();
        enemies.sort_by(|a, b| enemy_order(a, b, player.as_ref()));
        enemies.truncate(MAX_ENEMY_ROWS);
        Self { player, enemies }
    }

    /// 玩家位置（敌人行要拿它算距离）。
    pub fn player_position(&self) -> Option<Vec3> {
        self.player.as_ref().map(|row| row.position)
    }

    /// 取某一格的读数。
    pub fn of(&self, slot: PanelSlot) -> Option<&UnitRow> {
        match slot {
            PanelSlot::Player => self.player.as_ref(),
            PanelSlot::Enemy(index) => self.enemies.get(index),
        }
    }

    /// 这一格显示的名字（`PLAYER` / `ENEMY 1`）——多个敌人时得能分清是哪一个。
    pub fn name(&self, slot: PanelSlot) -> String {
        match slot {
            PanelSlot::Player => "PLAYER".to_string(),
            PanelSlot::Enemy(index) => format!("ENEMY {}", index + 1),
        }
    }

    /// HP 条宽。
    pub fn hp_percent(&self, slot: PanelSlot) -> Val {
        self.of(slot)
            .map(|row| bar_percent(row.health.current as f32, row.health.max as f32))
            .unwrap_or(Val::Percent(0.0))
    }

    /// 精力条宽（没有精力组件的单位显示空条）。
    pub fn stamina_percent(&self, slot: PanelSlot) -> Val {
        self.of(slot)
            .and_then(|row| row.stamina)
            .map(|stamina| bar_percent(stamina.current as f32, stamina.max as f32))
            .unwrap_or(Val::Percent(0.0))
    }

    /// HP 文本。
    pub fn hp_text(&self, slot: PanelSlot) -> String {
        self.of(slot)
            .map(|row| format!("HP {} / {}", row.health.current, row.health.max))
            .unwrap_or_default()
    }

    /// 精力文本。
    pub fn stamina_text(&self, slot: PanelSlot) -> String {
        self.of(slot)
            .map(|row| match row.stamina {
                Some(stamina) => format!("EN {} / {}", stamina.current, stamina.max),
                None => "EN -".to_string(),
            })
            .unwrap_or_default()
    }

    /// 洞察力读数行（玩家格没有可读项，返回空串）。
    pub fn insight_line(&self, slot: PanelSlot) -> String {
        self.of(slot)
            .and_then(|row| row.insight)
            .map(|insight| insight.line())
            .unwrap_or_default()
    }

    /// 状态行文本（只有敌人行会带上到玩家的距离）。
    pub fn state_line(&self, slot: PanelSlot) -> String {
        let Some(row) = self.of(slot) else {
            return String::new();
        };
        let distance = (slot.faction() == Faction::Enemy)
            .then(|| {
                self.player_position()
                    .map(|player| row.position.distance(player))
            })
            .flatten();
        row.state_line(&self.name(slot), distance)
    }
}

impl UnitRow {
    /// 状态行：`PLAYER · ready · cell (1,1) · arm 3`。
    ///
    /// `name` 由调用方给（面板知道这是"玩家"还是"第几个敌人"）；
    /// 本方法只负责把**这一行自己的数**排成一行字。
    pub fn state_line(&self, name: &str, to_player: Option<f32>) -> String {
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
        // 有效护甲（基础 + 装备加成）：装备一穿一脱，这个数立刻跟着变
        // 弹药（远程线）：**与精力并列的一条资源**，读作 `ammo 2/3`
        if let Some(ammo) = self.ammo {
            line.push_str(&format!(" · ammo {}/{}", ammo.current, ammo.max));
        }
        if let Some(armor) = self.armor {
            line.push_str(&format!(" · arm {armor}"));
        }
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
            ammo: Some(Ammo::new(3)),
            cell,
            position,
            slot: DecisionSlot::Idle { intent: None },
            dodging: false,
            parrying: false,
            airborne: false,
            tactic: None,
            armor: Some(1),
            insight: None,
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

        let line = enemy.state_line("ENEMY 1", Some(7.12));
        assert!(line.contains("ENEMY 1"), "名字由调用方给：{line}");
        assert!(
            line.contains("dodging"),
            "防御标记优先于 ready/busy：{line}"
        );
        assert!(line.contains("cell ( 3, 3)"), "{line}");
        assert!(line.contains("dist 7.1"), "{line}");
        assert!(line.contains("approach"), "{line}");
    }

    /// **有效护甲是面板上的一个读数**：装备一穿一脱，这个数跟着变。
    ///
    /// 这条守着"装备改动看得见"——没有它，装备只能靠日志猜。
    #[test]
    fn the_state_line_carries_the_effective_armor() {
        let mut unit = row(Faction::Player, Cell::new(0, 0), Vec3::ZERO);
        unit.armor = Some(3);
        assert!(
            unit.state_line("PLAYER", None).contains("arm 3"),
            "面板要显示有效护甲：{}",
            unit.state_line("PLAYER", None)
        );

        // 没有护甲组件的单位不显示这一段（而不是显示 arm 0）
        let mut without = row(Faction::Enemy, Cell::new(0, 0), Vec3::ZERO);
        without.armor = None;
        assert!(
            !without.state_line("ENEMY 1", None).contains("arm"),
            "缺组件就不显示"
        );
    }

    /// **弹药是面板上的一个读数**（资源分线之后玩家得看得见它）。
    #[test]
    fn the_state_line_carries_the_ammo() {
        let mut unit = row(Faction::Player, Cell::new(0, 0), Vec3::ZERO);
        unit.ammo = Some(Ammo::new(3));
        assert!(
            unit.state_line("PLAYER", None).contains("ammo 3/3"),
            "面板要显示弹药：{}",
            unit.state_line("PLAYER", None)
        );

        // 没有弹药组件的单位不显示这一段
        let mut without = row(Faction::Enemy, Cell::new(0, 0), Vec3::ZERO);
        without.ammo = None;
        assert!(
            !without.state_line("ENEMY 1", None).contains("ammo"),
            "缺组件就不显示"
        );
    }

    /// **洞察力读数拼得出来**：射程 / 打断抗性 / 战术各占一段。
    #[test]
    fn the_insight_line_carries_range_break_resistance_and_tactic() {
        let insight = insight_of(Some(2), Some(3), Some(Tactic::Approach)).unwrap();
        let line = insight.line();
        assert!(line.contains("range 2"), "{line}");
        assert!(
            line.contains("break 3"),
            "打断抗性是「该不该抢一手」的关键数：{line}"
        );
        assert!(line.contains("approach"), "{line}");
    }

    /// 缺的读数**整段不出现**（不留 `break ` 这种空段）。
    #[test]
    fn missing_insight_readings_are_left_out_entirely() {
        // 只有射程（比如敌人没在做事）
        let bare = insight_of(Some(1), None, None).unwrap();
        assert_eq!(bare.line(), "range 1", "没有前摇就没有打断抗性可读");

        // 什么都没有：整个读数都不该存在
        assert_eq!(insight_of(None, None, None), None);
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

        let player_line = panels.state_line(PanelSlot::Player);
        let enemy_line = panels.state_line(PanelSlot::Enemy(0));
        assert!(!player_line.contains("dist"), "{player_line}");
        assert!(enemy_line.contains("dist 6.0"), "{enemy_line}");
    }

    /// 某一格不在场时读数退回空值，而不是上一帧的残留。
    #[test]
    fn a_missing_slot_reads_as_empty() {
        let panels = UnitPanels::from_rows(&[row(Faction::Player, Cell::new(0, 0), Vec3::ZERO)]);

        let empty = PanelSlot::Enemy(0);
        assert_eq!(panels.state_line(empty), "");
        assert_eq!(panels.hp_text(empty), "");
        assert_eq!(panels.stamina_text(empty), "");
        assert_eq!(panels.hp_percent(empty), Val::Percent(0.0));
        assert_eq!(panels.stamina_percent(empty), Val::Percent(0.0));
        // 越界的名次同样退回空值，不 panic
        assert_eq!(panels.state_line(PanelSlot::Enemy(99)), "");
    }

    /// 没有精力组件的单位（例如场景里的纯装饰靶子）显示 `EN -` 而不是 0/0。
    #[test]
    fn a_unit_without_stamina_shows_a_dash() {
        let mut without = row(Faction::Player, Cell::new(0, 0), Vec3::ZERO);
        without.stamina = None;
        let panels = UnitPanels::from_rows(&[without]);

        assert_eq!(panels.stamina_text(PanelSlot::Player), "EN -");
        assert_eq!(panels.stamina_percent(PanelSlot::Player), Val::Percent(0.0));
    }

    /// **每个敌人各占一行，最近的在最前**，与遍历顺序无关。
    ///
    /// 这条取代了旧的"两个敌人只显示最近的那个"：现在两个敌人**都有行**，
    /// 而且**顺序由数据决定**（离玩家近的在前）——以前是"后遍历到的覆盖前面"，
    /// 而 ECS 查询顺序不保证，于是血条看起来自己在跳。
    #[test]
    fn every_enemy_gets_a_row_ordered_by_distance_whatever_the_iteration_order() {
        let player = row(Faction::Player, Cell::new(0, 0), Vec3::ZERO);
        let near = row(Faction::Enemy, Cell::new(1, 0), Vec3::new(2.0, 0.0, 0.0));
        let far = row(Faction::Enemy, Cell::new(5, 0), Vec3::new(10.0, 0.0, 0.0));

        for order in [
            vec![player, near, far],
            vec![player, far, near],
            vec![far, near, player],
        ] {
            let panels = UnitPanels::from_rows(&order);
            assert_eq!(panels.enemies.len(), 2, "两个敌人都该有行");
            assert_eq!(
                panels.enemies[0].cell,
                Cell::new(1, 0),
                "第 0 行必须是离玩家**最近**的那个（与遍历顺序无关）"
            );
            assert_eq!(panels.enemies[1].cell, Cell::new(5, 0), "远的那行在后");
            // 行名带名次：多个敌人时得能分清是哪一个
            assert_eq!(panels.name(PanelSlot::Enemy(0)), "ENEMY 1");
            assert_eq!(panels.name(PanelSlot::Enemy(1)), "ENEMY 2");
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
            a.enemies.iter().map(|row| row.cell).collect::<Vec<_>>(),
            b.enemies.iter().map(|row| row.cell).collect::<Vec<_>>(),
            "并列时两种顺序必须给出同一个次序"
        );
    }

    /// 单个敌人时照旧显示他（多敌人的改动不能影响只有一个的情况）。
    #[test]
    fn a_single_enemy_is_still_shown() {
        let panels = UnitPanels::from_rows(&[
            row(Faction::Player, Cell::new(0, 0), Vec3::ZERO),
            row(Faction::Enemy, Cell::new(3, 0), Vec3::new(6.0, 0.0, 0.0)),
        ]);
        assert_eq!(panels.enemies.len(), 1);
        assert_eq!(
            panels.of(PanelSlot::Enemy(0)).map(|row| row.cell),
            Some(Cell::new(3, 0))
        );
    }

    /// **敌人多于行数时只画前 N 个**（名次靠前的那些）。
    ///
    /// 行池大小是 [`MAX_ENEMY_ROWS`]：模型必须**按同一个数截断**，
    /// 否则多出来的敌人没有行可画（静默丢掉），而面板上看起来"敌人少了"。
    #[test]
    fn enemies_beyond_the_row_pool_are_dropped_by_rank() {
        let player = row(Faction::Player, Cell::new(0, 0), Vec3::ZERO);
        let mut rows = vec![player];
        // 造 MAX+2 个敌人，距离依次递增
        for index in 0..MAX_ENEMY_ROWS + 2 {
            let distance = (index + 1) as f32 * 2.0;
            rows.push(row(
                Faction::Enemy,
                Cell::new(index as i32 + 1, 0),
                Vec3::new(distance, 0.0, 0.0),
            ));
        }

        let panels = UnitPanels::from_rows(&rows);
        assert_eq!(
            panels.enemies.len(),
            MAX_ENEMY_ROWS,
            "最多画 {MAX_ENEMY_ROWS} 行"
        );
        // 留下的必须是**最近的那几个**
        for (index, enemy) in panels.enemies.iter().enumerate() {
            assert_eq!(
                enemy.position.x,
                (index + 1) as f32 * 2.0,
                "第 {index} 行应当是按距离排的第 {index} 个敌人"
            );
        }
    }
}
