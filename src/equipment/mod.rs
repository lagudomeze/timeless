//! # equipment — 装备领域
//!
//! 回答一个问题：**一个单位身上挂着什么，那些东西怎么改变它的战斗数值。**
//!
//! 完整的分层与设计推导见本域 [`components`] 的模块文档与
//! [`docs/equipment.md`](../../docs/equipment.md)。一句话：装备**不引入新的战斗机制**，
//! 它只是"往单位身上挂加成"的来源，命中公式与调度器照常读它们本来就读的组件。
//!
//! ## 文件分工
//!
//! | 文件 | 回答什么 |
//! | :--- | :--- |
//! | [`components`] | 槽位 / 物品 / 加成**是什么**（静态数据 + 组件） |
//! | [`domain`] | 「基础 + 加成」怎么算（**零 Bevy**，可脱离 App 单测） |
//! | [`relations`] | 「这件东西装在哪个槽」（`EquippedTo` / `EquippedItems`） |
//! | [`events`] | 与输入域、表现层的契约 |
//! | [`scene`] | 槽位与物品实体怎么组装（BSN 工厂） |
//! | [`systems`] | 校验、加成重算、穿脱 |

pub mod components;
pub mod domain;
pub mod events;
pub mod plugin;
pub mod relations;
pub mod scene;
pub mod systems;

pub use components::{EquipmentBonus, EquipmentSlot, Item, ItemBonus, ItemKind, SlotKind};
pub use domain::{
    effective_armor, effective_block_chance, effective_windup, sum_bonuses, weapon_damage,
};
pub use events::{EquipmentRefused, ToggleLoadout};
pub use plugin::EquipmentPlugin;
pub use relations::{EquippedItems, EquippedTo};
pub use scene::{equipment_slot_scene, item_scene};
pub use systems::{
    armor_of, block_chance_of, equip, recompute_equipment_bonus_system, toggle_loadout_system,
    unequip, validate_equipment_observer, weapon_timing,
};

/// 装备领域在 `Update` 中的系统集。
///
/// 它排在 [`CombatSet`](crate::combat::CombatSet) **之前**：加成必须在命中公式读它
/// 之前算好，否则这一帧的伤害用的还是上一帧的装备。
#[derive(bevy::prelude::SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EquipmentSet;
