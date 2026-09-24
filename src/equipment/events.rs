//! 装备域的跨领域契约。
//!
//! | 消息 / 事件 | 写 | 消费 |
//! | :--- | :--- | :--- |
//! | [`ToggleLoadout`] | [`crate::input`]（按键） | [`toggle_loadout_system`](super::toggle_loadout_system) |
//! | [`EquipmentRefused`] | 校验 Observer | `presentation` 的提示条 |
//!
//! 与项目里其它"被拒"的原因同一条通道：本域只说**为什么不行**，
//! 文案由表现层给（见 [`crate::presentation::hud::hint`]）。
//! 不用时间线的 `ActionBlocked`：装备不是一次"决策"，不该借用决策槽的拒绝理由。

use bevy::prelude::*;

use super::{ItemKind, SlotKind};

/// 请求在「全副武装」与「赤手空拳」之间切换（写：[`crate::input`]；消费：本域）。
///
/// 它是一次**测试性**的开关，不是玩法：这一版还没有"装备来源"（掉落 / 商店），
/// PC 直接带着一身起始装备出生，这条消息让玩家（和我们）能把穿 / 脱
/// 两个方向都走一遍。触发条件：出现真正的获取途径时，它应该被背包 UI 取代。
#[derive(Message, Debug, Clone, Copy)]
pub struct ToggleLoadout;

/// 一次装备操作被拒的原因（写：本域；消费：`presentation` 的提示条）。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipmentRefused {
    /// 这件东西装不进那个槽（类型不符）
    WrongSlot { item: ItemKind, slot: SlotKind },
    /// 目标槽位不存在（PC 还没组装出槽位实体）
    NoSuchSlot,
}
