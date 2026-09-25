//! 单位面板的**系统**：把世界摘成 [`UnitRow`]，交给 [`UnitPanels`] 算读数，
//! 再**只在读数变了**的时候写进 `Node` / `Text`。
//!
//! 这一层只做"取数 → 比对 → 写 UI"；文案与条宽怎么算全在 [`super::model`]。

use bevy::prelude::*;

use crate::ai::Tactic;
use crate::combat::Ammo;
use crate::combat::defense::{Dodging, Parrying, Stamina};
use crate::combat::{Armor, Faction, Health};
use crate::movement::{Cell, Jumping};
use crate::timeline::DecisionSlot;

use super::super::HudCache;
use super::model::{UnitPanels, UnitRow};
use super::scene::{PanelBar, PanelText, UnitPanel};

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
        Option<&'static Ammo>,
    ),
>;

/// 把 HP / EN / 状态行写进面板（玩家一格 + 敌人 **N** 行）。
///
/// 敌人那一列是**行池**：每帧按"离玩家最近"的名次把前几行填满，
/// 用不到的行藏起来（`Display::None`）——敌人数量变化因此**不增删实体**
/// （与时间轴色块池同一个做法）。
#[allow(clippy::too_many_arguments)]
pub fn update_unit_panels_system(
    units: UnitQuery<'_, '_>,
    mut cache: ResMut<HudCache>,
    // 三个查询都碰 `Node` / `Text`，用标记组件两两互斥（否则 Bevy 报 B0001）
    mut bars: Query<(&PanelBar, &mut Node), Without<UnitPanel>>,
    mut texts: Query<(&PanelText, &mut Text)>,
    mut rows: Query<(&UnitPanel, &mut Node), Without<PanelBar>>,
    // 决策槽：面板只读它，不写
    slots: Query<&DecisionSlot>,
    dodging: Query<(), With<Dodging>>,
    parrying: Query<(), With<Parrying>>,
    airborne: Query<(), With<Jumping>>,
    tactics: Query<&Tactic>,
    // 有效护甲 = 基础 + 装备加成：这里只问"是多少"，结构由 equipment 回答
    armors: Query<(&Armor, Option<&crate::equipment::EquipmentBonus>)>,
) {
    let rows_data: Vec<UnitRow> = units
        .iter()
        .map(
            |(entity, faction, health, cell, transform, stamina, ammo)| UnitRow {
                faction: *faction,
                health: *health,
                stamina: stamina.copied(),
                ammo: ammo.copied(),
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
    let panels = UnitPanels::from_rows(&rows_data);

    // 快照比对：这一帧与上一帧一模一样，就一个 UI 组件都不碰
    if cache.units.player == panels.player && cache.units.enemies == panels.enemies {
        return;
    }
    cache.units.player = panels.player;
    cache.units.enemies.clone_from(&panels.enemies);

    // 行池的显隐：有数据的行画出来，多余的行藏起来
    for (panel, mut node) in &mut rows {
        let occupied = panels.of(panel.slot).is_some();
        node.display = if occupied {
            Display::Flex
        } else {
            Display::None
        };
    }

    for (bar, mut node) in &mut bars {
        node.width = match bar {
            PanelBar::Hp(slot) => panels.hp_percent(*slot),
            PanelBar::En(slot) => panels.stamina_percent(*slot),
        };
    }

    for (label, mut text) in &mut texts {
        **text = match label {
            PanelText::Hp(slot) => panels.hp_text(*slot),
            PanelText::En(slot) => panels.stamina_text(*slot),
            PanelText::State(slot) => panels.state_line(*slot),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::hud::panels::PanelSlot;

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
            .spawn((PanelText::Hp(PanelSlot::Player), Text::new("")))
            .id();
        let hp_bar = app
            .world_mut()
            .spawn((PanelBar::Hp(PanelSlot::Player), Node::default()))
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
