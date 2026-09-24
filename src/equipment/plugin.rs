//! 装备域的插件：注册消息、Observer 与三个系统。

use bevy::prelude::*;

use super::EquipmentSet;
use super::events::{EquipmentRefused, ToggleLoadout};
use super::systems::{
    recompute_equipment_bonus_system, toggle_loadout_system, validate_equipment_observer,
};

/// 装备领域插件。
#[derive(Debug, Default)]
pub struct EquipmentPlugin;

impl Plugin for EquipmentPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ToggleLoadout>()
            .add_message::<EquipmentRefused>()
            // 校验只有一个入口：任何来源的 `EquippedTo` 都过它（见 `systems`）
            .add_observer(validate_equipment_observer)
            .add_systems(
                Update,
                // 穿脱先落地、加成随后重算——同帧内顺序即语义
                (toggle_loadout_system, recompute_equipment_bonus_system)
                    .chain()
                    .in_set(EquipmentSet),
            );
    }
}
