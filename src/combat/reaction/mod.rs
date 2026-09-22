//! # reaction — 反应系统（威胁检测 → 冻结世界 → 玩家可以改主意）
//!
//! 无回合模型里「谁什么时候动」是透明的，玩家因此**看得见即将落到自己头上的事**。
//! 本域把这件事变成机制：
//!
//! ```text
//! 每个 action 自己声明威胁覆盖的格（Threatens / TargetCell）
//!   ─▶ detect_threat_system：有**还没惊动过玩家**的威胁瞄着玩家
//!                             → 每帧断言 PauseRequest::Pause(THREAT)
//!   ─▶ mark_threatened_system：给这些来源打上 Threatened（**边沿触发**）
//!   ─▶ 世界冻结（Time<Virtual> 停表）：玩家可以撤销 / 换手 / 按空格走
//!   ─▶ 打上标记之后不再断言：要么玩家自己放行，要么那一手被打断 / 落地
//! ```
//!
//! **威胁声明在行动自己身上**，而不是由反应系统去猜：
//! 火球声明它飞过的格 + 落点，近战声明它扫过的扇形（用格近似）。
//! 因此新增攻击方式只要挂上 [`Threatens`]，反应系统一行不改。
//!
//! 飞行中的投射物同理：它们不再是"某条行动"，因此单独挂 [`TargetCell`]。
//!
//! ## 为什么是「边沿触发」
//!
//! 一次威胁只惊动玩家一次（[`Threatened`] 打在那个来源上）。于是玩家有两条出路：
//!
//! 1. **改主意**：撤销 / 换手 / 花 1 点 Focus 抢先手——正常反应；
//! 2. **按空格放行**（`PauseRequest::Toggle`）：清掉冻结，那一击照常落地。
//!
//! 第二条是刻意的：**忍受伤害也是一种决策**。旧实现靠"玩家那一手变了没有"推断
//! 表态，那个判据在**后摇 / 不可撤行动**期间永远为假（没槽可声明、也没行动可撤），
//! 玩家会被锁死在冻结里；边沿触发把"惊动过没有"记在威胁源自己身上，与玩家处于
//! 哪个阶段无关。

pub mod cells;
pub mod components;
pub mod systems;

pub use cells::{melee_arc_cells, trajectory_cells};
pub use components::{CounterSuggestion, ReactionSlot, TargetCell, Threatened, Threatens};
pub use systems::{
    ReactionAnswer, counter_suggestions, detect_threat_system, mark_threatened_system,
    resolve_reaction_system,
};
