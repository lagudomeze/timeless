//! # reaction — 反应系统（威胁检测 → 冻结世界 → 玩家可以改主意）
//!
//! 无回合模型里「谁什么时候动」是透明的，玩家因此**看得见即将落到自己头上的事**。
//! 本域把这件事变成机制：
//!
//! ```text
//! 每个 action 自己声明威胁覆盖的格（Threatens / TargetCell）
//!   ─▶ detect_threat_system：有威胁瞄准玩家且玩家还没表态
//!                             → 每帧断言 PauseRequest::Pause(THREAT)
//!   ─▶ 世界冻结（Time<Virtual> 停表）：玩家可以撤销 / 换手 / 用 Focus 抢先手
//!   ─▶ 回应过这次威胁（或威胁消失）→ 不再断言，原因下一帧自然消失
//! ```
//!
//! **威胁声明在行动自己身上**，而不是由反应系统去猜：
//! 火球声明它飞过的格 + 落点，近战声明它扫过的扇形（用格近似）。
//! 因此新增攻击方式只要挂上 [`Threatens`]，反应系统一行不改。
//!
//! 飞行中的投射物同理：它们不再是"某条行动"，因此单独挂 [`TargetCell`]。
//!
//! ## 为什么需要「回应」这一步
//!
//! 冻结是**互相**的：敌人也在冻结里，它那条威胁行动不会自己走到点。若一直冻着，
//! 玩家就会卡在"什么都做不了、威胁也永远不消失"的死锁里。因此窗口的语义是
//! 「威胁刚出现，等玩家表个态」：**玩家换了一手**（撤销或重新声明）就算回应过，
//! 世界照常流动，那一击该来就来。玩家不做任何事的话，世界就一直等他。

pub mod cells;
pub mod components;
pub mod systems;

pub use cells::{melee_arc_cells, trajectory_cells};
pub use components::{TargetCell, ThreatWindow, Threatens};
pub use systems::detect_threat_system;
