//! # skills — 技能：**静态定义**
//!
//! 技能是「这一手是什么」，行动是「这一次发生了什么」。本域只装**前者**：
//!
//! | | 住哪 | 能不能序列化 |
//! | :--- | :--- | :--- |
//! | 定义 [`AbilityDef`] | 本域（将来的 `.ron` 资产） | ✅ 只有数据 |
//! | 行动（载荷 + 执行器） | 各自的机制域（[`crate::movement`] / [`crate::combat`]） | ❌ 装着实体与计时 |
//!
//! **它不属于 `combat`**：技能是所有领域共享的静态目录，战斗只是最常读它的那个。
//! **移动 / 跳跃 / 翻滚也是技能**，和火球、横扫、招架走同一条路——没有"技能之外的行动"。
//!
//! ## 谁定义、谁交上来
//!
//! 数值归各域（`MOVE_TIMING` 在 `movement`、`FIREBALL_TIMING` 在 `combat`），
//! 各域在 `Startup` 用 [`RegisterAbility`] 把自己的定义交上来，本域只做聚合：
//! 技能栏、HUD、`can_cast`、反制建议都从这一份读，不在三个地方各写一遍花费。
//!
//! **没有全局派发器**：注册表回答"这一手是什么"，不回答"谁来物化它"——
//! 谁声明、谁物化（`movement` 物化移动、`combat` 物化攻击）。
//!
//! ## 文件地图
//!
//! | 文件 | 回答什么问题 |
//! | :--- | :--- |
//! | [`defs`] | 一条技能定义长什么样（`AbilityId` / `AbilityDef` / 标签 / 目标选择） |
//! | [`registry`] | 目录怎么建起来、怎么查（`SkillRegistry` + `RegisterAbility`） |
//! | [`plugin`] | 接线：注册资源与消息，每帧把新交上来的定义并进目录 |

pub mod defs;
pub mod plugin;
pub mod registry;

pub use defs::{
    AbilityCategory, AbilityDef, AbilityId, CombatTags, CounterCost, Requirement, ResourceCost,
    TargetSelector,
};
pub use plugin::SkillPlugin;
pub use registry::{Pools, RegisterAbility, SkillRegistry, can_cast};
