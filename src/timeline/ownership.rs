//! 行动实体的归属：这一手是谁的。
//!
//! 行动实体**没有 `Transform`**，它和行动者之间是纯逻辑关系，因此用自定义关系
//! （[`ActionOf`] / [`Actions`]）而不是 `ChildOf`：`ChildOf` 是**空间层级**，
//! 拿它表达归属会让行动实体混进 `Children`——「谁是谁的孩子」这句话就不再可读
//! （纸片、阴影、行动三种完全不同的东西挤在一个集合里）。规则的完整推导见
//! `docs/relations.md`。
//!
//! [`Actions`] 标了 `linked_spawn`，所以「行动者阵亡 / 重置 → 名下还没落地的行动
//! 跟着销毁」这条结构性保证照样成立：它原来靠 `Children` 拿，现在由本关系提供，
//! 行为不变（`src/lib.rs` 有整机用例守着）。
//!
//! 读写纪律：
//!
//! - **只改源那一侧**（往行动实体上写 [`ActionOf`]），集合 [`Actions`] 由 Bevy 的
//!   hook 自动同步，禁止手改集合；
//! - 读归属统一走 [`ActionOf::actor`]，不要直接摸字段。

use bevy::prelude::*;

/// 行动实体 → 行动者：「这一手是谁的」。
///
/// 派生 `FromTemplate` 是为了能在 BSN 场景里写 `ActionOf({actor})`
/// （`ChildOf` 走的是同一条路：靠 `#[entities]` 认出实体字段）；
/// 字段是 `pub` 才能被元组构造，但**读归属请走 [`ActionOf::actor`]**。
#[derive(Component, FromTemplate, Debug, Clone, Copy, PartialEq, Eq)]
#[relationship(relationship_target = Actions)]
pub struct ActionOf(#[entities] pub Entity);

impl ActionOf {
    /// 这一手的行动者。
    pub fn actor(&self) -> Entity {
        self.0
    }
}

/// 行动者 → 它名下**还没落地**的行动（由 Bevy 的 hook 维护，不要手改）。
///
/// `linked_spawn`：行动者被销毁时，这些行动跟着销毁。
#[derive(Component, Default, Debug)]
#[relationship_target(relationship = ActionOf, linked_spawn)]
pub struct Actions(Vec<Entity>);

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;

    /// 行动实体的场景工厂要 `spawn_scene`，因此得有 `AssetServer`。
    fn ownership_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .add_plugins(ScenePlugin);
        app
    }

    /// `bsn!` 能不能写**自定义**关系组件？
    ///
    /// M21 那次教训：我按 `Template` 的约束推断「`ChildOf` 进不了 `bsn!`」，
    /// 实测打脸。所以这条不靠推断，靠跑：`ActionOf({actor})` 用的是和
    /// `ChildOf({actor})` 同一套机制（`FromTemplate` + `#[entities]`）。
    #[test]
    fn a_scene_can_declare_the_custom_relationship() {
        let mut app = ownership_app();
        let actor = app.world_mut().spawn_empty().id();
        let action = app
            .world_mut()
            .spawn_scene(bsn! { ActionOf({actor}) })
            .expect("场景里写自定义关系组件应当能解析")
            .id();

        assert_eq!(
            app.world().get::<ActionOf>(action).map(ActionOf::actor),
            Some(actor),
            "写进场景的归属应当真的落到行动实体上"
        );
        assert_eq!(
            app.world().get::<Actions>(actor).map(|a| a.0.clone()),
            Some(vec![action]),
            "行动者身上的集合应当由 hook 自动维护"
        );
    }

    /// 归属换成自定义关系后，「行动者没了，行动也得没」这条保证不能丢。
    #[test]
    fn despawning_the_actor_takes_its_actions_with_it() {
        let mut app = ownership_app();
        let actor = app.world_mut().spawn_empty().id();
        let action = app
            .world_mut()
            .spawn_scene(bsn! { ActionOf({actor}) })
            .unwrap()
            .id();

        app.world_mut().despawn(actor);

        assert!(
            app.world().get_entity(action).is_err(),
            "行动者被销毁时，名下行动应当跟着销毁（linked_spawn）"
        );
    }

    /// 一个行动者可以同时有多手没落地的行动，集合不丢人。
    #[test]
    fn one_actor_can_own_several_pending_actions() {
        let mut app = ownership_app();
        let actor = app.world_mut().spawn_empty().id();
        let first = app
            .world_mut()
            .spawn_scene(bsn! { ActionOf({actor}) })
            .unwrap()
            .id();
        let second = app
            .world_mut()
            .spawn_scene(bsn! { ActionOf({actor}) })
            .unwrap()
            .id();

        let mut owned = app.world().get::<Actions>(actor).unwrap().0.clone();
        owned.sort();
        let mut expected = vec![first, second];
        expected.sort();
        assert_eq!(owned, expected);
    }
}
