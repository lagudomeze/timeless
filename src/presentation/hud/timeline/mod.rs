//! 顶部时间轴：把「谁在什么时候出手」画成一排色块。
//!
//! 一个功能分三层，各自一个文件——**模型不认识 Bevy 实体，系统不判断几何**：
//!
//! | 文件 | 回答什么 | 能不能脱离 App 单测 |
//! | :--- | :--- | :--- |
//! | `model` | 每个色块画在哪、谁在候场（纯函数 + 快照类型） | ✅ 纯数据进、纯数据出 |
//! | `scene` | 开局的 UI 夹具与节点标记组件 | 只建实体，用 `World` 即可 |
//! | `system` | 取数 → 比对快照 → 写 `Node` / `Text` | 需要 App（本文件的 `tests`） |
//!
//! 设计语义（色块 = 占用、刻线 = 结算、按单位分道、候场区）写在 `model` 的文首。

mod model;
mod scene;
mod system;

pub use model::{
    BLOCK_POOL_PER_LANE, LANE_GAP, LANE_HEIGHT, LANE_POOL, MIN_BLOCK_WIDTH, STAGING_WIDTH,
    TICK_POOL, TICK_SECONDS, TimelineCache, TimelineModel, WINDOW_SECONDS, build_model,
};
pub use scene::{
    TimelineBlock, TimelineBlockLabel, TimelineBlockMark, TimelineLane, TimelineLaneLabel,
    TimelineReadyChip, TimelineReadyLabel, TimelineStateLabel, spawn_timeline,
};
pub use system::update_timeline_system;
