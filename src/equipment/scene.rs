//! 装备域的实体组装：槽位与物品。
//!
//! 槽位是 PC 的物理延伸（`ChildOf(pc)`），物品装上去时再挂 `ChildOf(slot)`——
//! 于是"跟着 PC 动"与"装在哪个槽"分别是两件事，卸下只断后者（见
//! [`relations`](super::relations)）。
//!
//! 物品带一个很小的可见方块：这一版没有装备栏图标，**看得见才验得动**
//! （穿上 / 脱下时画面上要真的多一件、少一件）。尺寸与位置是这个用途的占位。

use bevy::prelude::*;

use super::components::{EquipmentSlot, Item, ItemKind, SlotKind};
use super::relations::EquippedTo;

/// 物品占位方块的颜色（按种类区分，一眼能看出身上挂着什么）。
pub const SWORD_COLOR: Color = Color::srgb(0.85, 0.86, 0.92);
/// 见 [`SWORD_COLOR`]。
pub const SHIELD_COLOR: Color = Color::srgb(0.72, 0.55, 0.32);
/// 见 [`SWORD_COLOR`]。
pub const MAIL_COLOR: Color = Color::srgb(0.55, 0.60, 0.70);

impl ItemKind {
    /// 占位方块的边长（世界单位）。
    pub fn visual_size(self) -> f32 {
        match self {
            Self::Sword => 0.12,
            Self::Shield => 0.42,
            Self::Mail => 0.30,
        }
    }

    /// 占位方块挂在槽位的哪个偏移上（避开单位纸片本身）。
    pub fn visual_offset(self) -> Vec3 {
        match self {
            Self::Sword => Vec3::new(0.45, 0.55, 0.0),
            Self::Shield => Vec3::new(-0.45, 0.50, 0.0),
            Self::Mail => Vec3::new(0.0, 0.75, 0.0),
        }
    }

    /// 占位方块的颜色。
    pub fn visual_color(self) -> Color {
        match self {
            Self::Sword => SWORD_COLOR,
            Self::Shield => SHIELD_COLOR,
            Self::Mail => MAIL_COLOR,
        }
    }
}

/// 一个槽位实体：**没有渲染内容**，只是"这个槽在这儿"（PC 的子节点）。
///
/// 它不该有可见的网格——装备的"看得见"由物品自己负责，槽位是纯结构。
/// 挂 `ChildOf(pc)` 是为了"跟着动"这条空间事实（见 `docs/relations.md` 第三节）。
///
/// ⚠️ `Transform` 必须是**局部的零偏移**：单位根节点就是**脚底**
/// （见 [`crate::spawn::unit`]），所以槽位挂在它下面、零偏移，正好落在脚下。
/// 这里若写世界坐标，会和父级的位置**再叠一次**——实机探针抓到过这个：
/// PC 在 `(3,-1,1)`、卸下的物品却到了 `(6,-2,2)`。
pub fn equipment_slot_scene(slot: SlotKind, owner: Entity) -> impl Scene {
    bsn! {
        Name("EquipmentSlot")
        EquipmentSlot { slot: {slot} }
        ChildOf({owner})
        Transform::default()
        Visibility::default()
    }
}

/// 一件物品实体：装在 `slot` 上，位置由 `ChildOf(slot)` 推着走。
///
/// 两件事都在模板里写死：`EquippedTo`（逻辑归属）与 `ChildOf`（空间附着）。
///
/// ⚠️ **装上时 `Transform` 必须是 `ZERO`（局部坐标）**：它跟着槽位（→ PC → 脚底）
/// 走，`Transform` 是相对父级的偏移。把世界坐标写进去会**再叠一次**父级的位置——
/// 实机探针抓到过这个：PC 在 `(3,-1,1)`，卸下时物品却落在 `(6,-2,2)`。
/// 世界坐标只在**卸下那一刻**才有意义（见
/// [`unequip`](super::unequip)）。
pub fn item_scene(kind: ItemKind, slot: Entity) -> impl Scene {
    let size = kind.visual_size();
    let offset = kind.visual_offset();
    bsn! {
        Name("Item")
        Item { kind: {kind} }
        EquippedTo({slot})
        ChildOf({slot})
        // 局部零偏移：跟着槽位走。**不要**写世界坐标（见本函数文档）
        Transform::default()
        Visibility::Visible
        Children [
            (
                Name("ItemVisual")
                Mesh3d(asset_value(Cuboid::new(size, size, size)))
                MeshMaterial3d<StandardMaterial>(asset_value(StandardMaterial {
                    base_color: {kind.visual_color()},
                    unlit: true,
                    ..default()
                }))
                Transform {
                    translation: {offset},
                }
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;

    fn scene_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .add_plugins(ScenePlugin)
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>();
        app
    }

    /// **场景工厂真的把标记挂上了**：漏一行不报编译错，
    /// 只会表现成"装备永远不生效 / 卸下时物品失踪"。
    #[test]
    fn the_item_scene_attaches_its_components_and_children() {
        let mut app = scene_app();
        // 槽位要先真的存在：`ChildOf` 的关系 hook 会往**父实体**上写 `Children`，
        // 拿 `Entity::PLACEHOLDER` 当槽位会直接 panic（"Entity not yet spawned"）。
        let owner = app.world_mut().spawn_empty().id();
        let slot = app
            .world_mut()
            .spawn_scene(equipment_slot_scene(SlotKind::MainHand, owner))
            .unwrap()
            .id();
        app.world_mut().flush();
        let item = app
            .world_mut()
            .spawn_scene(item_scene(ItemKind::Sword, slot))
            .unwrap()
            .id();
        app.world_mut().flush();

        assert_eq!(
            app.world().get::<EquipmentSlot>(slot).map(|slot| slot.slot),
            Some(SlotKind::MainHand),
            "槽位要记得自己收什么"
        );
        assert_eq!(
            app.world().get::<ChildOf>(slot).map(ChildOf::parent),
            Some(owner),
            "槽位是单位的物理延伸（跟着动）"
        );
        assert_eq!(
            app.world().get::<Item>(item).map(|item| item.kind),
            Some(ItemKind::Sword),
            "物品必须带上自己的种类（加成 / 校验都读它）"
        );
        assert_eq!(
            app.world().get::<EquippedTo>(item).map(EquippedTo::slot),
            Some(slot),
            "物品必须记得自己在哪个槽（卸下的判据）"
        );
        assert_eq!(
            app.world().get::<ChildOf>(item).map(ChildOf::parent),
            Some(slot),
            "物品必须跟着槽位动（空间附着）"
        );
        // 槽位与物品的 `Transform` 必须是**局部零偏移**：单位根就是脚底，
        // 写世界坐标会和父级再叠一次（实机探针抓到过错位）
        assert_eq!(
            app.world().get::<Transform>(slot).unwrap().translation,
            Vec3::ZERO,
            "槽位是局部零偏移（它不是「脚底的位置」，那是 PC 根节点的职责）"
        );
        assert_eq!(
            app.world().get::<Transform>(item).unwrap().translation,
            Vec3::ZERO,
            "物品是局部零偏移（跟着槽位走，世界坐标由父级给）"
        );
    }

    /// 槽位带**自己的种类**：校验 Observer 靠它判"这件东西装不装得进"。
    #[test]
    fn each_slot_kind_labels_itself() {
        for slot in SlotKind::ALL {
            let label = slot.label();
            assert!(!label.is_empty(), "{slot:?} 要有可读的短名");
        }
    }
}
