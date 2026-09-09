//! # 战斗重置模块
//!
//! R 键 → [`ResetBattle`] 消息 → 清场（单位 / 攻击实体）→ 重新生成单位场景。
//! 消息与消费系统同属本文件所在领域（input 只负责翻译按键）。

pub mod input;
pub mod systems;

pub use input::reset_input_system;
pub use systems::{ResetBattle, reset_system};
