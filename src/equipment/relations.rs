//! 物品与槽位的关系：**装在哪个槽**。
//!
//! 用自定义关系而不是 `ChildOf`（规则的完整推导见
//! [`docs/relations.md`](../../docs/relations.md)）：`EquippedTo` 管**逻辑归属**
//! （"装在主手"），`ChildOf` 管**空间附着**（"跟着 PC 动"）——两件事。
//!
//! 分得很清的好处是**卸下**这一步读得通：断掉 [`EquippedTo`] 之后物品仍然是活实体
//! （可以掉在地上 / 进背包），而它当初的"跟着动"由槽位的 `ChildOf` 负责，
//! 物品自己只在**装上的时候**额外挂一个 `ChildOf(slot)`。
//!
//! 读写纪律与 [`ActionOf`](crate::timeline::ActionOf) 相同：**只改源那一侧**，
//! 集合由 Bevy 的 hook 维护。

use bevy::prelude::*;

/// 物品 → 槽位：「这件东西装在哪个槽」。
///
/// 派生 `FromTemplate` 是为了能在 BSN 场景里写 `EquippedTo({slot})`；
/// 读归属请走 [`EquippedTo::slot`]。
#[derive(Component, FromTemplate, Debug, Clone, Copy, PartialEq, Eq)]
#[relationship(relationship_target = EquippedItems)]
pub struct EquippedTo(#[entities] pub Entity);

impl EquippedTo {
    /// 这件物品所在的槽位实体。
    pub fn slot(&self) -> Entity {
        self.0
    }
}

/// 槽位 → 它上面装着的物品（由 Bevy 的 hook 维护，不要手改）。
///
/// **没有 `linked_spawn`**：槽位被销毁（PC 阵亡 / 重置）时物品不该跟着消失，
/// 那样会连带丢掉玩家的东西；物品的清理归它自己的生命周期。
#[derive(Component, Default, Debug)]
#[relationship_target(relationship = EquippedTo)]
pub struct EquippedItems(Vec<Entity>);

impl EquippedItems {
    /// 槽里现在的物品（正常是 0 或 1 件，用切片表达"可能有多个"这件事本身）。
    pub fn items(&self) -> &[Entity] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;

    fn relation_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .add_plugins(ScenePlugin);
        app
    }

    /// 与 [`ActionOf`](crate::timeline::ActionOf) 同一条实测结论：`bsn!` 认的是
    /// `FromTemplate` + `#[entities]`，自己定义的关系照写。
    #[test]
    fn a_scene_can_declare_the_equipped_relationship() {
        let mut app = relation_app();
        let slot = app.world_mut().spawn_empty().id();
        let item = app
            .world_mut()
            .spawn_scene(bsn! { EquippedTo({slot}) })
            .expect("场景里写装备关系应当能解析")
            .id();

        assert_eq!(
            app.world().get::<EquippedTo>(item).map(EquippedTo::slot),
            Some(slot),
            "写进场景的归属应当真的落到物品上"
        );
        assert_eq!(
            app.world()
                .get::<EquippedItems>(slot)
                .map(|s| s.items().to_vec()),
            Some(vec![item]),
            "槽位身上的集合应当由 hook 自动维护"
        );
    }

    /// **卸下 ≠ 销毁**：断掉归属之后物品仍然活着（"掉在地上"这件事的前提）。
    ///
    /// 这条钉的是本文件与 `ActionOf` 的分工——行动实体没有 `Transform`，
    /// 归属断了就该消失；物品是**真的东西**，归属断了它还在世界上。
    #[test]
    fn unequipping_leaves_the_item_alive() {
        let mut app = relation_app();
        let slot = app.world_mut().spawn_empty().id();
        let item = app
            .world_mut()
            .spawn_scene(bsn! { EquippedTo({slot}) })
            .unwrap()
            .id();

        app.world_mut().entity_mut(item).remove::<EquippedTo>();
        app.world_mut().flush();

        assert!(
            app.world().get_entity(item).is_ok(),
            "卸下的物品还活着（要掉在地上 / 进背包），不该被级联销毁"
        );
        // 集合空了之后 Bevy 会把 `EquippedItems` 整个摘掉（`None` = 一件都没有），
        // 所以"清空"的判据是"没有非空的集合"
        assert!(
            app.world()
                .get::<EquippedItems>(slot)
                .is_none_or(|items| items.items().is_empty()),
            "槽位上的集合应当被同步清空"
        );
    }

    /// 槽位被销毁时物品**不**跟着销毁：集合没标 `linked_spawn`。
    #[test]
    fn despawning_the_slot_does_not_take_the_item_with_it() {
        let mut app = relation_app();
        let slot = app.world_mut().spawn_empty().id();
        let item = app
            .world_mut()
            .spawn_scene(bsn! { EquippedTo({slot}) })
            .unwrap()
            .id();

        app.world_mut().despawn(slot);

        assert!(
            app.world().get_entity(item).is_ok(),
            "PC 阵亡不该把玩家的东西一起销毁"
        );
    }
}
