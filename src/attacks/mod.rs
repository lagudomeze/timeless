//! # 攻击生成层
//!
//! 每种攻击一个场景工厂 + 可选的生成系统。生成只负责“摆出实体 + 挂效果
//! 组件”，命中与结算交给 `combat` 通用流水线，互不感知。

pub mod arrow;
pub mod input;
pub mod melee;
pub mod messages;
pub mod systems;

pub use arrow::arrow_scene;
pub use input::player_fire_input_system;
pub use melee::melee_scene;
pub use messages::{FireCommand, MeleeCommand};
pub use systems::{player_fire_arrow_system, player_melee_system};
