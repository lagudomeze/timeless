//! 命中结算子域插件：扣血系统与打断观察者。

use bevy::prelude::*;

use super::FormulaSet;

use super::{apply_physical_hits_system, interrupt_observer};

/// 命中结算：防御判定 → 护甲 → 扣血 → 触发打断。
pub struct FormulaPlugin;

impl Plugin for FormulaPlugin {
    fn build(&self, app: &mut App) {
        app
            // BRP 诊断锚点：护甲是命中结算读的数（`attributes` 是纯数据、没有插件）
            .register_type::<crate::combat::Armor>()
            // 打断是命中触发的战斗裁决，落地在本域自己收
            .add_observer(interrupt_observer)
            .add_systems(Update, apply_physical_hits_system.in_set(FormulaSet));
    }
}
