//! # 战斗日志模块
//!
//! 订阅 `DamageEvent` / `DeathEvent`，把可读文本写入 [`BattleLog`] 资源，
//! 供将来的 HUD / 复盘 UI 读取。当前先以 console 输出验证。

pub mod systems;

pub use systems::{BattleLog, battle_log_system};
