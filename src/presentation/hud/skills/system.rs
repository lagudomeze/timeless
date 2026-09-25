//! 技能栏的**系统**：读选中项 / 悬停项 / 精力，比对快照，再写槽位外观与 tooltip。
//!
//! 配色怎么选、文案怎么拼全在 [`super::model`]；这一层只负责"取数 → 比对 → 写 UI"。

use bevy::prelude::*;

use crate::combat::Faction;
use crate::combat::attack::MenuSelection;
use crate::combat::defense::Stamina;
use crate::combat::reaction::ReactionSlot;

use super::super::HudCache;
use super::model::{
    SkillBarCache, affordability, badge_color, badge_text, counter_hints, slot_bg, slot_border,
    tooltip_text,
};
use super::scene::{SkillBadge, SkillSlot, SkillTooltip, SkillTooltipText};

/// 每帧刷新槽位外观、角标与 tooltip。
pub fn update_skill_bar_system(
    selection: Res<MenuSelection>,
    // 反制建议住在被威胁的玩家身上（`ReactionSlot`）：HUD 只读它、不认识 `CounterCost`。
    // 表现层仍按 `Faction` 找人（它看的是"哪个阵营的单位"，不是"谁在输入"）。
    players: Query<(&Faction, &Stamina, Option<&ReactionSlot>)>,
    mut cache: ResMut<HudCache>,
    mut slots: Query<(
        &SkillSlot,
        &Interaction,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
    mut badges: Query<(&SkillBadge, &mut Text, &mut TextColor), Without<SkillTooltipText>>,
    mut tooltips: Query<&mut Node, (With<SkillTooltip>, Without<SkillSlot>)>,
    mut tooltip_texts: Query<&mut Text, (With<SkillTooltipText>, Without<SkillBadge>)>,
) {
    let player = players
        .iter()
        .find(|(faction, _, _)| **faction == Faction::Player);
    let stamina = player.map(|(_, stamina, _)| stamina.current);
    let suggestions = player
        .and_then(|(_, _, slot)| slot)
        .map(|slot| slot.suggestions.as_slice());

    // 悬停要读 `Interaction`（很便宜），但写节点前先比对快照
    let hovered = slots
        .iter()
        .find(|(_, interaction, _, _)| **interaction == Interaction::Hovered)
        .map(|(slot, _, _, _)| slot.index);
    let snapshot = SkillBarCache {
        selected: selection.index(),
        hovered,
        affordable: affordability(stamina),
        counters: counter_hints(suggestions),
    };
    if cache.skills == snapshot {
        return;
    }
    cache.skills = snapshot.clone();

    for (slot, interaction, mut background, mut border) in &mut slots {
        let affordable = snapshot.affordable[slot.index];
        let selected = slot.index == snapshot.selected;
        let counter = snapshot.counters[slot.index];
        *background = BackgroundColor(slot_bg(affordable, selected, counter));
        *border = BorderColor::all(slot_border(
            *interaction == Interaction::Hovered,
            selected,
            affordable,
            counter,
        ));
    }

    for (badge, mut text, mut color) in &mut badges {
        **text = badge_text(badge.index);
        *color = TextColor(badge_color(snapshot.affordable[badge.index]));
    }

    for mut tooltip in &mut tooltips {
        tooltip.display = if hovered.is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Some(index) = snapshot.hovered {
        for mut text in &mut tooltip_texts {
            **text = tooltip_text(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::attack::{SKILLS, SkillKind};
    use crate::combat::reaction::CounterSuggestion;
    use crate::presentation::hud::skills::model::{SLOT_BG, SLOT_BG_LOCKED, SLOT_BORDER};

    fn skill_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<HudCache>()
            .insert_resource(MenuSelection::default())
            .add_systems(Update, update_skill_bar_system);
        app
    }

    fn spawn_slot(app: &mut App, index: usize) -> Entity {
        app.world_mut()
            .spawn((
                SkillSlot { index },
                Interaction::default(),
                BackgroundColor(SLOT_BG),
                BorderColor::all(SLOT_BORDER),
            ))
            .id()
    }

    /// tooltip 的显示状态（`Query::single` 需要 `&mut World`，所以这里收 `&mut App`）。
    fn tooltip_display(app: &mut App) -> Display {
        let mut query = app
            .world_mut()
            .query_filtered::<&Node, With<SkillTooltip>>();
        query.single(app.world_mut()).unwrap().display
    }

    #[test]
    fn affordable_and_selected_slots_get_distinct_tints() {
        let mut app = skill_app();
        let player = app
            .world_mut()
            .spawn((Faction::Player, Stamina::new(5)))
            .id();
        let melee = spawn_slot(&mut app, 1); // 免费技能
        let fireball = spawn_slot(&mut app, 2); // 2 精力
        let attack = spawn_slot(&mut app, 0); // 默认选中（2 精力）

        app.update();

        let background =
            |app: &App, entity: Entity| app.world().get::<BackgroundColor>(entity).unwrap().0;
        assert_ne!(
            background(&app, attack),
            background(&app, melee),
            "选中的槽位应当有独立底色"
        );
        assert_eq!(
            background(&app, melee),
            background(&app, fireball),
            "都没选中时用同一底色"
        );

        // 精力清零后，2 精力的技能整体压暗；免费的近战不变
        app.world_mut()
            .entity_mut(player)
            .get_mut::<Stamina>()
            .unwrap()
            .current = 0;
        app.update();
        assert_eq!(background(&app, fireball), SLOT_BG_LOCKED);
        assert_eq!(background(&app, melee), SLOT_BG, "免费技能永远可用");
    }

    #[test]
    fn badges_show_the_energy_cost() {
        let mut app = skill_app();
        let badge = app
            .world_mut()
            .spawn((
                SkillBadge { index: 2 },
                Text::new(""),
                TextColor(Color::WHITE),
            ))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<Text>(badge).unwrap().0,
            SKILLS[2].cost.amount().to_string()
        );
    }

    #[test]
    fn hovering_a_slot_shows_its_tooltip() {
        let mut app = skill_app();
        let slot = spawn_slot(&mut app, 3);
        app.world_mut().spawn((SkillTooltip, Node::default()));
        let text = app
            .world_mut()
            .spawn((SkillTooltipText, Text::new("")))
            .id();

        app.update();
        assert_eq!(
            tooltip_display(&mut app),
            Display::None,
            "没悬停时 tooltip 应当隐藏"
        );

        *app.world_mut().get_mut::<Interaction>(slot).unwrap() = Interaction::Hovered;
        app.update();
        assert!(
            app.world().get::<Text>(text).unwrap().0.contains("ROLL"),
            "悬停应当显示该技能的说明"
        );
        assert_eq!(
            tooltip_display(&mut app),
            Display::Flex,
            "悬停时 tooltip 应当显示"
        );
    }

    /// **威胁窗口一开，能当反制的那几手就亮起来**——这是 M26 未接完的那一半。
    ///
    /// 之前 `suggestions` 算出来了却没人读：玩家被威胁冻结时，技能栏看不出
    /// "我现在能拿什么挡"。这条钉住整条链：`ReactionSlot` → `counter_hints` → 槽位底色。
    #[test]
    fn a_counter_suggestion_lights_up_the_skill_that_can_answer_it() {
        use crate::combat::reaction::ReactionSlot;
        use crate::skills::{AbilityId, CounterCost};

        let mut app = skill_app();
        let roll_index = SKILLS
            .iter()
            .position(|def| def.kind == SkillKind::Roll)
            .expect("技能栏里应当有翻滚");
        let player = app
            .world_mut()
            .spawn((Faction::Player, Stamina::new(5)))
            .id();
        let roll = spawn_slot(&mut app, roll_index);
        let fireball = spawn_slot(&mut app, 2);

        app.update();
        let background =
            |app: &App, entity: Entity| app.world().get::<BackgroundColor>(entity).unwrap().0;
        let before = background(&app, roll);

        // 开窗：威胁压过来，翻滚被列为可用反制
        app.world_mut().entity_mut(player).insert(ReactionSlot {
            threat: player,
            suggestions: vec![CounterSuggestion {
                ability: AbilityId::Roll,
                cost: CounterCost::Free,
                affordable: true,
            }],
            resolved: false,
        });
        app.update();

        assert_ne!(
            background(&app, roll),
            before,
            "能当反制的那一手应当变色，否则玩家看不出该按哪个"
        );
        assert_ne!(
            background(&app, roll),
            background(&app, fireball),
            "只有被建议的那一手亮，其它手不变"
        );
    }

    /// 选中项 / 悬停 / 可负担性都没变时，技能栏一帧都不写。
    #[test]
    fn skill_bar_is_left_alone_when_nothing_changes() {
        let mut app = skill_app();
        let player = app
            .world_mut()
            .spawn((Faction::Player, Stamina::new(5)))
            .id();
        let slot = spawn_slot(&mut app, 2);

        app.update();
        // 拿哨兵色当探针：数据没变时系统不该把它盖回去
        app.world_mut().get_mut::<BackgroundColor>(slot).unwrap().0 = Color::WHITE;
        app.update();
        assert_eq!(
            app.world().get::<BackgroundColor>(slot).unwrap().0,
            Color::WHITE,
            "数据没变就不该重写槽位"
        );

        // 精力清零 → 2 精力的技能变不可负担，必须重画
        app.world_mut().get_mut::<Stamina>(player).unwrap().current = 0;
        app.update();
        assert_eq!(
            app.world().get::<BackgroundColor>(slot).unwrap().0,
            SLOT_BG_LOCKED
        );
    }
}
