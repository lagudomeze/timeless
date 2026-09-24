//! 顶部时间轴：把「谁在什么时候出手」画成一排色块。
//!
//! 一个功能分三层，各自一个文件——**模型不认识 Bevy 实体，系统不判断几何**：
//!
//! | 文件 | 回答什么 | 能不能脱离 App 单测 |
//! | :--- | :--- | :--- |
//! | `model` | 每个色块画在哪、谁在候场（纯函数 + 快照类型） | ✅ 纯数据进、纯数据出 |
//! | `readout` | 悬停时那一行字怎么拼（纯函数 + `TimelineHover` 事实） | ✅ |
//! | `scene` | 开局的 UI 夹具与节点标记组件 | 只建实体，用 `World` 即可 |
//! | `system` | 取数 → 比对快照 → 写 `Node` / `Text` / 指示圈 | 需要 App（本文件的 `tests`） |
//!
//! 设计语义（色块 = 占用、刻线 = 结算、按单位分道、候场区、悬停读数）写在
//! `model` / `readout` 的文首。

mod model;
mod readout;
mod scene;
mod system;

pub use model::{
    BLOCK_POOL_PER_LANE, LANE_GAP, LANE_HEIGHT, LANE_POOL, MIN_BLOCK_WIDTH, STAGING_WIDTH,
    TICK_POOL, TICK_SECONDS, TimelineCache, TimelineModel, WINDOW_SECONDS, build_model,
};
pub use readout::{
    ActionReadout, HoveredAction, TimelineFocusRing, TimelineHover, TimelineReadout,
    TimelineReadoutText, interrupt_label, readout_line,
};
pub use scene::{
    TimelineBlock, TimelineBlockLabel, TimelineBlockMark, TimelineLane, TimelineLaneLabel,
    TimelineReadyChip, TimelineReadyLabel, TimelineStateLabel, spawn_timeline,
    spawn_timeline_focus_ring,
};
pub use system::{
    TimelineLayout, TimelineReadoutState, update_timeline_focus_ring_system,
    update_timeline_readout_system, update_timeline_system,
};
