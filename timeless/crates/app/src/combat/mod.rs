//! # 战斗机制（引擎侧）
//!
//! 每种机制只拥有一个小组件 + 将来一个专属系统，机制之间不聚合：
//!
//! - [`damage`]：伤害数值（命中后扣 `Health`）；
//! - [`range`]：攻击射程（命中判定，纯函数见 `domain::combat::within_range`）；
//! - [`frame`]：出招速度（决定前摇时长 / 同刻先后）；
//! - [`impact`]：破势对抗（同刻互击时高者打断低者）。
//!
//! 纯逻辑入口统一在 `domain::combat`，这里只定义 ECS 数据载体。

pub mod damage;
pub mod frame;
pub mod impact;
pub mod range;

pub use damage::Damage;
pub use frame::AttackFrame;
pub use impact::Impact;
pub use range::AttackRange;
