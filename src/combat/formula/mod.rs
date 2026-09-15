//! 伤害：纯公式、纯防御逻辑、一条**没有中间态**的扣血链。
//!
//! | 模块 | 职责 | 依赖 |
//! | :--- | :--- | :--- |
//! | [`domain`] | 防御判定 / 反制伤害（**纯函数**） | 零 Bevy |
//! | [`systems`] | 护甲公式 + 每种伤害类型的命中系统 | Bevy |
//! | [`events`] | `DamageEvent` | Bevy 消息 |
//!
//! **没有两阶段裁决**：伤害是纯减法（可交换），谁先谁后不影响结果，
//! 因此不需要 `Arbitration` 快照，也不需要"阶段 1 只读 / 阶段 2 落地"。
//!
//! **伤害类型不是枚举**：一种伤害 = 一个组件（`PhysicalDamage` / 将来的
//! `FireDamage`…）+ 一个把它变成 [`DamageEvent`] 的系统 + 挂载它的工厂。
//! 生命值链路（`DamageEvent` → `apply_damage_system` → `DeathEvent`）对所有人共用。

pub mod domain;
pub mod events;
pub mod systems;

pub use domain::{DefenseState, counter_damage, resolve_defense};
pub use events::DamageEvent;
pub use systems::{apply_physical_hits_system, physical_damage};
