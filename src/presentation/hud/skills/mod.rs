//! 技能栏：图标按钮 + 角标 + 悬停 tooltip。
//!
//! 与时间轴 / 面板同一套三层分法：
//!
//! | 文件 | 回答什么 | 能不能脱离 App 单测 |
//! | :--- | :--- | :--- |
//! | `model` | 槽位该是什么色、tooltip 写什么（纯函数） | ✅ 纯数据进、纯数据出 |
//! | `scene` | 槽位与 tooltip 的 UI 夹具、节点标记组件 | 只建实体 |
//! | `system` | 取数 → 比对快照 → 写 `Node` / `Text` | 需要 App |
//!
//! 槽位显示三件事：**图标**（占了什么位置）、**角标**（现在是精力消耗，等
//! `Cooldowns` 落地后同一个节点改显示剩余 CD）、**tooltip**（名称 / 消耗 / 前摇后摇 /
//! 威力）。不可负担的槽位整体压暗，选中的槽位加金色描边。

mod model;
mod scene;
mod system;

pub use model::{SkillBarCache, icon_path, tooltip_text};
pub use scene::{
    ICON_SIZE, SLOT_SIZE, SkillBadge, SkillSlot, SkillTooltip, SkillTooltipText, skill_bar,
};
pub use system::update_skill_bar_system;
