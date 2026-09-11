//! 伤害与裁决：纯公式、纯裁决、两阶段结算。
//!
//! | 模块 | 职责 | 依赖 |
//! | :--- | :--- | :--- |
//! | [`domain`] | 三层裁决 / 防御判定 / 反制伤害（**纯函数**） | 零 Bevy |
//! | [`systems`] | 护甲公式（纯函数） | 零 Bevy |
//! | [`resolution`] | 两阶段结算（只读裁决 → 统一落地） | Bevy |
//! | [`events`] / [`types`] | `DamageEvent` / 伤害类型 | Bevy 消息 |

pub mod domain;
pub mod events;
pub mod resolution;
pub mod systems;
pub mod types;

pub use domain::{
    AttackStats, DefenseState, HitOrder, HitResult, Side, counter_damage, resolve_attack,
    resolve_combat, resolve_defense,
};
pub use events::DamageEvent;
pub use resolution::{Arbitration, CombatResult, phase1_arbitrate_system, phase2_apply_system};
pub use systems::physical_damage;
pub use types::DamageType;
