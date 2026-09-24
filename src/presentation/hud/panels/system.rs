//! 单位面板的**系统**：把世界摘成 [`UnitRow`]，交给 [`UnitPanels`] 算读数，
//! 再**只在读数变了**的时候写进 `Node` / `Text`。
//!
//! 这一层只做"取数 → 比对 → 写 UI"；文案与条宽怎么算全在 [`super::model`]。

use bevy::prelude::*;

use crate::ai::Tactic;
use crate::combat::defense::{Dodging, Parrying, Stamina};
use crate::combat::{Armor, Faction, Health};
use crate::movement::{Cell, Jumping};
use crate::timeline::DecisionSlot;

use super::super::HudCache;
use super::model::{UnitPanels, UnitRow};
use super::scene::{PanelBar, PanelText};

/// 单位快照查询（实体 + 阵营 + 血量 + 格 + 位姿 + 精力）。
type UnitQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Faction,
        &'static Health,
        &'static Cell,
        &'static Transform,
        Option<&'static Stamina>,
    ),
>;

/// 把 HP / EN / 状态行写进面板。
#[allow(clippy::too_many_arguments)]
pub fn update_unit_panels_system(
    units: UnitQuery<'_, '_>,
    mut cache: ResMut<HudCache>,
    mut bars: Query<(&PanelBar, &mut Node)>,
    mut texts: Query<(&PanelText, &mut Text)>,
    // 决策槽：面板只读它，不写
    slots: Query<&DecisionSlot>,
    dodging: Query<(), With<Dodging>>,
    parrying: Query<(), With<Parrying>>,
    airborne: Query<(), With<Jumping>>,
    tactics: Query<&Tactic>,
    // 有效护甲 = 基础 + 装备加成：这里只问"是多少"，结构由 equipment 回答
    armors: Query<(&Armor, Option<&crate::equipment::EquipmentBonus>)>,
) {
    let rows: Vec<UnitRow> = units
        .iter()
        .map(
            |(entity, faction, health, cell, transform, stamina)| UnitRow {
                faction: *faction,
                health: *health,
                stamina: stamina.copied(),
                cell: *cell,
                position: transform.translation,
                slot: slots.get(entity).copied().unwrap_or_default(),
                dodging: dodging.get(entity).is_ok(),
                parrying: parrying.get(entity).is_ok(),
                airborne: airborne.get(entity).is_ok(),
                tactic: tactics.get(entity).ok().copied(),
                armor: armors
                    .get(entity)
                    .ok()
                    .map(|(base, bonus)| crate::equipment::armor_of(base.0, bonus)),
            },
        )
        .collect();
    let panels = UnitPanels::from_rows(&rows);

    // 快照比对：这一帧与上一帧一模一样，就一个 UI 组件都不碰
    if cache.units.rows == panels.rows {
        return;
    }
    cache.units.rows = panels.rows;

    for (bar, mut node) in &mut bars {
        node.width = match bar {
            PanelBar::Hp(faction) => panels.hp_percent(*faction),
            PanelBar::En(faction) => panels.stamina_percent(*faction),
        };
    }

    for (label, mut text) in &mut texts {
        **text = match label {
            PanelText::Hp(faction) => panels.hp_text(*faction),
            PanelText::En(faction) => panels.stamina_text(*faction),
            PanelText::State(faction) => panels.state_line(*faction),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 数据没变时面板整帧不写；数值一变就必须跟着变。
    #[test]
    fn panels_are_left_alone_when_nothing_changes() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<HudCache>()
            .add_systems(Update, update_unit_panels_system);
        let player = app
            .world_mut()
            .spawn((
                Faction::Player,
                Health::new(50),
                Cell::new(1, 1),
                Transform::from_xyz(3.0, 0.0, 3.0),
                Stamina::new(5),
            ))
            .id();
        let hp_text = app
            .world_mut()
            .spawn((PanelText::Hp(Faction::Player), Text::new("")))
            .id();
        let hp_bar = app
            .world_mut()
            .spawn((PanelBar::Hp(Faction::Player), Node::default()))
            .id();

        app.update();
        assert_eq!(app.world().get::<Text>(hp_text).unwrap().0, "HP 50 / 50");
        assert_eq!(
            app.world().get::<Node>(hp_bar).unwrap().width,
            Val::Percent(100.0)
        );

        // 哨兵值：数据没变时系统不该把它盖回去
        app.world_mut().get_mut::<Text>(hp_text).unwrap().0 = "SENTINEL".to_string();
        app.update();
        assert_eq!(
            app.world().get::<Text>(hp_text).unwrap().0,
            "SENTINEL",
            "数据没变就不该重写 UI"
        );

        // 掉一半血：文本与条宽都要跟上
        app.world_mut().get_mut::<Health>(player).unwrap().current = 25;
        app.update();
        assert_eq!(app.world().get::<Text>(hp_text).unwrap().0, "HP 25 / 50");
        assert_eq!(
            app.world().get::<Node>(hp_bar).unwrap().width,
            Val::Percent(50.0)
        );
    }
}
