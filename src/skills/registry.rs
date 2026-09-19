//! 技能目录：怎么建起来、怎么查。
//!
//! **只有一个目录**（本资源），**每个域定义自己的技能**：各域在 `Startup` 写一条
//! [`RegisterAbility`]，目录在 `Update` 把它们并进来。
//!
//! 为什么走消息而不是"各域往别人的资源里塞"：这是铁律里「别人的内部状态不许碰」
//! 的标准解法（见 `docs/domain.md` 第二节）。
//!
//! 为什么在 `Update` 合并而不是 `Startup`：注册是一次性的突发写入，但
//! **跨插件的 `Startup` 顺序不该成为一件要记在心里的事**——放在 `Update` 里，
//! 谁在哪一阶段交上来都收得到（合并按 id 覆盖，重复注册是幂等的）。

use bevy::prelude::*;

use super::defs::{AbilityDef, AbilityId};

/// 各域把自己的静态定义交上来（写：机制域；消费：本域）。
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct RegisterAbility(pub AbilityDef);

/// 技能目录：`AbilityId` → [`AbilityDef`]，**保留注册顺序**（菜单顺序就是它）。
#[derive(Resource, Debug, Default)]
pub struct SkillRegistry(Vec<AbilityDef>);

impl SkillRegistry {
    /// 一条都没有（启动第一帧之前的状态）。
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// 目录里的全部技能（顺序 = 注册顺序）。
    pub fn all(&self) -> &[AbilityDef] {
        &self.0
    }

    /// 按 id 查。
    pub fn get(&self, id: AbilityId) -> Option<&AbilityDef> {
        self.0.iter().find(|def| def.id == id)
    }

    /// 按 id 查，缺失即 panic（目录是启动期就定型的静态数据，缺了就是接线漏了）。
    pub fn expect(&self, id: AbilityId) -> &AbilityDef {
        self.get(id)
            .unwrap_or_else(|| panic!("技能目录里没有 {}：注册漏了", id.label()))
    }

    /// 当前精力负担得起的技能。
    pub fn affordable(&self, stamina: u32) -> impl Iterator<Item = &AbilityDef> {
        self.0.iter().filter(move |def| def.cost <= stamina)
    }

    /// 并进一条定义：同 id **覆盖并留在原位**（重复注册不改菜单顺序）。
    pub fn register(&mut self, def: AbilityDef) {
        match self.0.iter_mut().find(|existing| existing.id == def.id) {
            Some(existing) => *existing = def,
            None => self.0.push(def),
        }
    }
}

/// 把这一帧交上来的定义并进目录。
pub fn apply_registrations_system(
    mut requests: MessageReader<RegisterAbility>,
    mut registry: ResMut<SkillRegistry>,
) {
    for RegisterAbility(def) in requests.read() {
        registry.register(*def);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::defs::{AbilityCategory, CombatTags, TargetSelector};
    use crate::timeline::ActionTiming;

    fn def(id: AbilityId, cost: u32) -> AbilityDef {
        AbilityDef {
            id,
            category: AbilityCategory::Movement,
            timing: ActionTiming::new(0.1, 0.2, 1),
            targeting: TargetSelector::SelfOnly,
            cost,
            combat: CombatTags::COMMITTED,
            power: 0,
        }
    }

    fn registry_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<SkillRegistry>()
            .add_message::<RegisterAbility>()
            .add_systems(Update, apply_registrations_system);
        app
    }

    /// 各域交上来的定义，下一帧就能查到。
    #[test]
    fn published_definitions_become_lookups() {
        let mut app = registry_app();
        assert!(app.world().resource::<SkillRegistry>().is_empty());

        app.world_mut()
            .write_message(RegisterAbility(def(AbilityId::Move, 0)));
        app.update();

        let registry = app.world().resource::<SkillRegistry>();
        assert_eq!(
            registry.get(AbilityId::Move).map(|def| def.id),
            Some(AbilityId::Move)
        );
        assert_eq!(registry.get(AbilityId::Jump), None, "没交上来的查不到");
    }

    /// 重复注册覆盖内容、**不动菜单顺序**。
    #[test]
    fn re_registering_keeps_the_menu_order() {
        let mut app = registry_app();
        app.world_mut()
            .write_message(RegisterAbility(def(AbilityId::Move, 0)));
        app.world_mut()
            .write_message(RegisterAbility(def(AbilityId::Jump, 0)));
        app.update();

        app.world_mut()
            .write_message(RegisterAbility(def(AbilityId::Move, 2)));
        app.update();

        let registry = app.world().resource::<SkillRegistry>();
        let order: Vec<AbilityId> = registry.all().iter().map(|def| def.id).collect();
        assert_eq!(order, vec![AbilityId::Move, AbilityId::Jump]);
        assert_eq!(registry.expect(AbilityId::Move).cost, 2, "覆盖应当生效");
    }

    /// 负担得起只看花费。
    #[test]
    fn affordability_filters_by_cost() {
        let mut registry = SkillRegistry::default();
        registry.register(def(AbilityId::Melee, 0));
        registry.register(def(AbilityId::Roll, 1));
        registry.register(def(AbilityId::Fireball, 2));

        let free: Vec<AbilityId> = registry.affordable(0).map(|def| def.id).collect();
        assert_eq!(free, vec![AbilityId::Melee]);
        let all: Vec<AbilityId> = registry.affordable(9).map(|def| def.id).collect();
        assert_eq!(all.len(), 3);
    }
}
