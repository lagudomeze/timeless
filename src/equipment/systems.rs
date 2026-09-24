//! 装备域的应用层：校验、加成重算、穿脱。
//!
//! 三层分工与项目其它域一致：纯逻辑在 [`super::domain`]（零 Bevy），
//! 实体组装在 [`super::scene`]，本文件只做"读世界 → 判定 → 落地"。
//!
//! ## 为什么加成是**每帧重算**而不是"穿上时加、卸下时减"
//!
//! "基础 + 加成"这个形状让重算变成**幂等**的：把 `EquipmentBonus` 整个清零再算一遍，
//! 结果一定相同。于是不需要记住"上次加了多少"，也就不存在两处记账对不上的可能。
//! 重算的输入（槽位 → 物品）是世界里的关系本身，**没有第二份真相**。
//! 单位数量是个位数，每帧算一遍的代价可以忽略；而且只在**真的变了**的时候写组件。

use bevy::prelude::*;

use crate::timeline::ActionTiming;

use super::components::{EquipmentBonus, EquipmentSlot, Item, ItemBonus, ItemKind, SlotKind};
use super::domain::{effective_armor, effective_block_chance, effective_windup, sum_bonuses};
use super::events::{EquipmentRefused, ToggleLoadout};
use super::relations::EquippedTo;
use super::scene::item_scene;

/// 校验：**槽只收对的东西**。
///
/// 无论装备是被玩家点上去的、被脚本塞的、还是被 AI 装的，都过这一个 Observer——
/// 所以"类型校验"只有一份实现，不会在采购 / UI / 组装三处各写一遍。
/// 不符合当场退回（移除 [`EquippedTo`]），并给出一条拒绝理由让 HUD 能说清楚。
pub fn validate_equipment_observer(
    insert: On<Insert, EquippedTo>,
    mut commands: Commands,
    items: Query<&Item>,
    slots: Query<&EquipmentSlot>,
    equipped: Query<&EquippedTo>,
    mut refused: MessageWriter<EquipmentRefused>,
) {
    let item_entity = insert.entity;
    // `On` 解引用后就是事件本身（`Insert` 只有 `entity` 一个字段），
    // 因此归属要从实体上读回来——`Insert` 在组件写入**之后**触发，读得到。
    let Ok(equipped) = equipped.get(item_entity) else {
        return;
    };
    let slot_entity = equipped.slot();

    let Ok(slot) = slots.get(slot_entity) else {
        refused.write(EquipmentRefused::NoSuchSlot);
        commands.entity(item_entity).remove::<EquippedTo>();
        return;
    };
    let Ok(item) = items.get(item_entity) else {
        // 没有 `Item` 的东西不是装备：不该挂在槽上
        commands.entity(item_entity).remove::<EquippedTo>();
        return;
    };
    if item.kind.slot() != slot.slot {
        refused.write(EquipmentRefused::WrongSlot {
            item: item.kind,
            slot: slot.slot,
        });
        commands.entity(item_entity).remove::<EquippedTo>();
    }
}

/// 重算每个单位的 [`EquipmentBonus`]：槽位 → 物品 → 加成求和。
///
/// 读取侧（命中公式 / 攻击执行器 / 声明系统）因此都只读**一个**组件，
/// 不必各自去遍历槽位与物品——"装备怎么变成数值"这件事只有这一处实现。
///
/// **只在真的变了时写**：`EquipmentBonus` 是 `PartialEq`，值相同就不碰组件，
/// 免得每帧的写入噪声把别人基于变更检测的优化毁掉。
pub fn recompute_equipment_bonus_system(
    mut commands: Commands,
    slots: Query<(Entity, &EquipmentSlot, &ChildOf)>,
    items: Query<(&Item, &EquippedTo)>,
    bonuses: Query<&EquipmentBonus>,
) {
    // 单位 → 它名下的加成之和。**从槽位出发**，所以"有槽位但一件没穿"的单位
    // 也会拿到一份零加成（否则上一身的残留会留在组件里）。
    let mut by_unit: Vec<(Entity, ItemBonus)> = Vec::new();
    for (slot_entity, _, parent) in &slots {
        let unit = parent.parent();
        let index = match by_unit.iter().position(|(owner, _)| *owner == unit) {
            Some(index) => index,
            None => {
                by_unit.push((unit, ItemBonus::default()));
                by_unit.len() - 1
            }
        };
        if let Some((item, _)) = items
            .iter()
            .find(|(_, equipped)| equipped.slot() == slot_entity)
        {
            by_unit[index].1 = sum_bonuses([by_unit[index].1, item.kind.bonus()]);
        }
    }

    for (unit, total) in by_unit {
        let total = EquipmentBonus(total);
        let current = bonuses.get(unit).copied().unwrap_or_default();
        if current == total {
            continue; // 没变就不写（别把变更检测的噪声撒给别人）
        }
        if let Ok(mut entity) = commands.get_entity(unit) {
            entity.insert(total);
        }
    }
}

/// 穿上一件装备（`equip` 的实体侧）：产出物品实体并挂到槽位上。
///
/// 校验不在这里——由 [`validate_equipment_observer`] 兜底，所以这个函数只管"挂上"。
/// 它**不写世界坐标**：物品装上时的 `Transform` 是局部零偏移
/// （见 [`item_scene`](super::scene::item_scene) 的说明）。
pub fn equip(commands: &mut Commands, slot: Entity, kind: ItemKind) -> Entity {
    commands.spawn_scene(item_scene(kind, slot)).id()
}

/// 卸下一件装备：断逻辑归属 + 断空间附着，并**把它现在所在的世界坐标写回
/// `Transform`**，于是它留在原地。
///
/// ## 为什么第 ③ 步不能省（设计稿里"最容易忘的一步"）
///
/// 物品装上时的 `Transform` 是**局部零偏移**（见
/// [`item_scene`](super::scene::item_scene)），它之所以在 PC 身上完全靠父级链
/// （槽位 → PC → 脚底）推着。一旦解除 `ChildOf`，父级变换不再作用，
/// 那个零偏移就**字面地**变成了世界坐标——物品会跳到世界原点。
///
/// 所以这里必须显式地把"它现在在哪儿"（调用方传进来的
/// [`GlobalTransform`]）落到 `Transform` 上。**传错来源会静默错位**：
/// 曾经传成"PC 的世界坐标"，而那时物品的局部偏移也是世界坐标，
/// 两者叠起来物品落在 `2×` PC 位置（实机探针抓到：PC `(3,-1,1)`、物品 `(6,-2,2)`）。
///
/// 物品**不销毁**：它是一件真的东西（这就是 [`EquippedTo`] 不用 `ChildOf` 的理由）。
/// 卸下之后它落在原地、重新可见，等着被捡起来。
pub fn unequip(commands: &mut Commands, item: Entity, world_position: Vec3) {
    commands
        .entity(item)
        .remove::<EquippedTo>()
        .remove::<ChildOf>()
        .insert(Transform::from_translation(world_position))
        .insert(Visibility::Visible);
}

/// 找到某个单位在某个槽上的物品（正常是 0 或 1 件）。
fn item_in_slot(slot: Entity, items: &Query<(Entity, &Item, &EquippedTo)>) -> Option<Entity> {
    items
        .iter()
        .find(|(_, _, equipped)| equipped.slot() == slot)
        .map(|(entity, _, _)| entity)
}

/// `T`：在「全副武装」与「赤手空拳」之间切换。
///
/// ⚠️ **它是调试开关，不是玩法**：这一版还没有装备来源（掉落 / 商店），
/// PC 直接带着起始装备出生；这个开关让玩家（和我们）把穿 / 脱两个方向都走一遍，
/// 顺便把"卸下的物品还在世界上"这条结构保证真的跑起来。
/// 出现真正的获取途径时，它该被背包 UI 取代（见 [`ToggleLoadout`]）。
pub fn toggle_loadout_system(
    mut requests: MessageReader<ToggleLoadout>,
    mut commands: Commands,
    players: Query<Entity, With<crate::timeline::InputDriven>>,
    slots: Query<(Entity, &EquipmentSlot, &ChildOf)>,
    items: Query<(Entity, &Item, &EquippedTo)>,
    // 卸下时要知道物品现在在**哪儿**：局部向量会自动跟着父级平移，所以
    // 局部 `Transform` 拿不到"当前世界位置"，必须读 `GlobalTransform`
    globals: Query<&GlobalTransform>,
) {
    if requests.read().next().is_none() {
        return;
    }
    let Some(player) = players.iter().next() else {
        return;
    };
    // 这个 PC 的槽位（槽位是 PC 的子实体）
    let own_slots: Vec<(Entity, SlotKind)> = slots
        .iter()
        .filter(|(_, _, parent)| parent.parent() == player)
        .map(|(entity, slot, _)| (entity, slot.slot))
        .collect();
    if own_slots.is_empty() {
        return;
    }

    let equipped: Vec<Entity> = own_slots
        .iter()
        .filter_map(|(slot, _)| item_in_slot(*slot, &items))
        .collect();

    if equipped.is_empty() {
        // 穿上起始装备（物品的局部 `Transform` 是零，跟着槽位走）
        for (slot, kind) in &own_slots {
            equip(&mut commands, *slot, ItemKind::for_slot(*kind));
        }
        return;
    }

    // 全部卸下：物品留在原地（写回它现在的世界坐标，见 `unequip`）
    for item in equipped {
        let at = globals
            .get(item)
            .map(|transform| transform.translation())
            .unwrap_or(Vec3::ZERO);
        unequip(&mut commands, item, at);
    }
}

/// HUD 读数用：这个单位现在的有效护甲（基础 + 装备加成）。
///
/// 让表现层不必知道"基础 / 加成"的结构——它只管问"有效值是多少"。
pub fn armor_of(base: i32, bonus: Option<&EquipmentBonus>) -> i32 {
    effective_armor(base, bonus.map(|bonus| bonus.armor()).unwrap_or(0))
}

/// 见 [`armor_of`]。
pub fn block_chance_of(base: f32, bonus: Option<&EquipmentBonus>) -> f32 {
    effective_block_chance(base, bonus.map(|bonus| bonus.block_chance()).unwrap_or(0.0))
}

/// 武器改动作节奏：**只给偏移，不整条覆盖**（设计稿第七节的"乙"）。
///
/// 与第五节的"基础 + 加成"是**同一个形状**：技能的基础节奏住在 `config/actions.ron`，
/// 武器只说自己快多少。于是同一条技能定义在有 / 没武器时都成立，不需要第二套心智模型。
///
/// ⚠️ **只有攻击动作吃武器节奏**：移动 / 翻滚 / 跳跃的节奏与手上的东西无关，
/// 让一把剑改掉翻滚的前摇是没有道理的。调用方（三个攻击声明系统）因此显式地用它。
pub fn weapon_timing(base: ActionTiming, bonus: Option<&EquipmentBonus>) -> ActionTiming {
    ActionTiming {
        windup: effective_windup(
            base.windup,
            bonus.map(|bonus| bonus.windup_delta()).unwrap_or(0.0),
        ),
        ..base
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::Armor;
    use crate::combat::defense::BlockChance;
    use bevy::scene::ScenePlugin;

    /// 装备系统的整机（最小）App：三个系统 + Observer 都在位。
    fn equipment_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .add_plugins(ScenePlugin)
            // `GlobalTransform` 是**派生数据**（由 `TransformPlugin` 每帧传播）：
            // 没有它，卸下的物品永远读到 `(0,0,0)`，落点测试就测不到真东西
            .add_plugins(bevy::transform::TransformPlugin)
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .add_message::<ToggleLoadout>()
            .add_message::<EquipmentRefused>()
            .add_observer(validate_equipment_observer)
            .add_systems(
                Update,
                (toggle_loadout_system, recompute_equipment_bonus_system).chain(),
            );
        app
    }

    /// 一个"玩家"：基础护甲 1、基础格挡 0（与 `spawn::unit` 的起始值同形）。
    fn spawn_player(app: &mut App) -> Entity {
        app.world_mut()
            .spawn((
                crate::timeline::InputDriven,
                Armor(1),
                BlockChance(0.0),
                Transform::from_xyz(2.0, 0.0, 2.0),
            ))
            .id()
    }

    /// 给玩家建一个槽位（槽位是玩家的子实体，摆在玩家脚底）。
    fn spawn_slot(app: &mut App, owner: Entity, kind: SlotKind) -> Entity {
        let slot = app
            .world_mut()
            .spawn_scene(crate::equipment::equipment_slot_scene(kind, owner))
            .unwrap()
            .id();
        app.world_mut().flush();
        slot
    }

    /// 起始装备真的挂在槽位上，并且**加成真的落到单位身上**。
    ///
    /// 这条是整条链路的地基：装备不是"发下去就算了"，它必须变成
    /// `EquipmentBonus`，而读取侧（命中公式）只认那一个组件。
    #[test]
    fn equipping_gear_reaches_the_units_bonus_component() {
        let mut app = equipment_app();
        let player = spawn_player(&mut app);
        let slot = spawn_slot(&mut app, player, SlotKind::MainHand);

        equip(&mut app.world_mut().commands(), slot, ItemKind::Sword);
        app.update(); // 命令落地 + 重算

        let bonus = app
            .world()
            .get::<EquipmentBonus>(player)
            .copied()
            .expect("穿上装备之后单位应当有加成组件");
        assert_eq!(bonus.armor(), 0, "剑不给护甲");
        assert_eq!(bonus.damage(), 1, "剑给 1 点伤害");
        assert!(
            (bonus.windup_delta() + 0.05).abs() < 1e-6,
            "剑把前摇提前 0.05s"
        );
        assert_eq!(armor_of(1, Some(&bonus)), 1, "有效护甲 = 基础 1 + 剑的 0");
    }

    /// **卸下之后加成一分不剩**（"基础 + 加成"最要紧的那条：
    /// 不需要记住上次加了多少，重算是幂等的）。
    ///
    /// 顺带钉住**落点**：卸下的物品停在 PC 脚下——不是世界原点，也不是被父级
    /// 变换叠加后的错位点。实机探针抓到过那个错位（PC 在 `(3,-1,1)`、
    /// 物品却落在 `(6,-2,2)`，正好是父级位置又叠了一遍）。
    #[test]
    fn unequipping_leaves_no_bonus_behind_and_drops_it_at_the_feet() {
        let mut app = equipment_app();
        let player = spawn_player(&mut app); // 站在 (2,0,2)
        let slot = spawn_slot(&mut app, player, SlotKind::Armor);
        let item = equip(&mut app.world_mut().commands(), slot, ItemKind::Mail);
        app.update();
        assert_eq!(
            app.world().get::<EquipmentBonus>(player).unwrap().armor(),
            1,
            "身甲给 1 点护甲"
        );

        let at = app
            .world()
            .get::<GlobalTransform>(item)
            .unwrap()
            .translation();
        unequip(&mut app.world_mut().commands(), item, at);
        app.update();
        app.update(); // 让 `GlobalTransform` 的传播跑完，才能验世界坐标

        let bonus = app.world().get::<EquipmentBonus>(player).copied();
        assert_eq!(
            bonus.map(|bonus| bonus.armor()),
            Some(0),
            "卸下之后加成归零（不是留着上一次的数字）"
        );
        assert!(app.world().get_entity(item).is_ok(), "卸下的物品还在世界上");
        assert!(
            app.world().get::<ChildOf>(item).is_none(),
            "卸下要断掉空间附着（否则物品还跟着 PC 走）"
        );
        let dropped = app
            .world()
            .get::<GlobalTransform>(item)
            .unwrap()
            .translation();
        assert_eq!(
            dropped,
            Vec3::new(2.0, 0.0, 2.0),
            "卸下的物品停在 PC 脚下（世界原点 / 被叠加的错位点都是 bug）"
        );
    }

    /// **校验 Observer 真的在拦**：盾装不进主手，当场退回并给出理由。
    #[test]
    fn the_wrong_item_is_refused_and_dropped_from_the_slot() {
        let mut app = equipment_app();
        let player = spawn_player(&mut app);
        let main_hand = spawn_slot(&mut app, player, SlotKind::MainHand);
        // 硬造一个"盾在主手"的错误状态：直接挂 `EquippedTo`（任何来源都该过 Observer）
        let shield = app
            .world_mut()
            .spawn((Item {
                kind: ItemKind::Shield,
            },))
            .id();
        app.world_mut()
            .entity_mut(shield)
            .insert(crate::equipment::EquippedTo(main_hand));
        app.update();

        assert!(
            app.world()
                .get::<crate::equipment::EquippedTo>(shield)
                .is_none(),
            "装错槽的东西应当被当场退回（否则槽位上会挂着一件它不该收的东西）"
        );
        let refused = app.world().resource::<Messages<EquipmentRefused>>();
        assert!(!refused.is_empty(), "拒绝要给出理由，HUD 才有话可说");
    }

    /// `T` 开关：穿上 → 三件都在；再按一次 → 全脱掉，而且**加成退到零**。
    ///
    /// 这条把"脱"这个方向也真的跑一遍——只测穿不测脱的话，
    /// `unequip` 里那三步（断归属 / 断空间 / 写回坐标）任何一步漏掉都不会被发现。
    #[test]
    fn the_loadout_toggle_puts_gear_on_and_takes_it_off() {
        let mut app = equipment_app();
        let player = spawn_player(&mut app);
        for slot in SlotKind::ALL {
            spawn_slot(&mut app, player, slot);
        }
        app.update();
        assert!(
            app.world().get::<EquipmentBonus>(player).is_none(),
            "一件没穿时重算给出零加成，组件值就是默认值、不必写上"
        );

        // 按一次：穿上全套
        app.world_mut().write_message(ToggleLoadout);
        app.update();
        app.update();
        let worn = app.world().get::<EquipmentBonus>(player).copied().unwrap();
        assert_eq!(worn.armor(), 2, "盾 1 + 甲 1");
        assert_eq!(worn.damage(), 1, "剑 1");
        assert!(
            (worn.block_chance() - 0.35).abs() < 1e-6,
            "盾给 0.35 格挡率（这是格挡管线第一次真的有来源）"
        );

        // 再按一次：全脱掉
        app.world_mut().write_message(ToggleLoadout);
        app.update();
        app.update();
        let bare = app.world().get::<EquipmentBonus>(player).copied().unwrap();
        assert_eq!(
            bare,
            EquipmentBonus::default(),
            "赤手空拳时加成应当全零（回到基础值）"
        );
        assert_eq!(armor_of(1, Some(&bare)), 1, "有效护甲回到基础值 1");
    }

    /// 槽位上没有东西而单位身上**已经**有残留加成时，重算要把它清掉。
    ///
    /// 这条守着"从槽位出发"那个实现选择：如果只遍历物品，那么"装备被直接销毁"
    /// （不经过 `unequip`）会在单位身上留下一份永远不会消失的加成。
    #[test]
    fn recomputing_clears_a_stale_bonus_when_nothing_is_worn() {
        let mut app = equipment_app();
        let player = spawn_player(&mut app);
        spawn_slot(&mut app, player, SlotKind::Armor);
        // 造一份残留：上一帧穿着甲，而物品已经没了
        app.world_mut()
            .entity_mut(player)
            .insert(EquipmentBonus(ItemBonus {
                armor: 7,
                ..ItemBonus::default()
            }));

        app.update();

        assert_eq!(
            app.world().get::<EquipmentBonus>(player).unwrap().armor(),
            0,
            "一件没穿时残留的加成必须被清掉"
        );
    }
}
