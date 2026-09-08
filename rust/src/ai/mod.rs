//! # 敌人行为模块
//!
//! 世界空间下的持续决策：距离玩家远则接近（写自己的 `Velocity`），
//! 进入攻击距离且冷却结束则生成攻击场景。不感知回合 / 时间线。

pub mod components;
pub mod systems;

pub use components::{AttackCooldown, EnemyBrain};
pub use systems::enemy_ai_system;
