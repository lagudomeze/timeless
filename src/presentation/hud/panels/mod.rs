//! 单位状态面板：头像 + HP / EN 条 + 状态行（左下 = 玩家，右下 = 敌人）。
//!
//! 与时间轴同一套三层分法——**模型不认识实体，系统不写文案**：
//!
//! | 文件 | 回答什么 | 能不能脱离 App 单测 |
//! | :--- | :--- | :--- |
//! | `model` | 状态行 / 条宽怎么算（`UnitRow` / `UnitPanels` + 纯函数） | ✅ 纯数据进、纯数据出 |
//! | `scene` | 面板的 UI 夹具与节点标记组件 | 只建实体 |
//! | `system` | 取数 → 比对快照 → 写 `Node` / `Text` | 需要 App |
//!
//! 「滑动条」是**只读进度条**：条长 = 数值比例，HUD 不改任何游戏状态
//! （[`model::bar_fraction`]）。

mod model;
mod scene;
mod system;

pub use model::{
    MAX_ENEMY_ROWS, PanelSlot, UnitPanelCache, UnitPanels, UnitRow, bar_fraction, bar_percent,
    tactic_label,
};
pub use scene::{
    ENEMY_ROW_GAP, ENEMY_ROW_MIN_HEIGHT, PANEL_HEIGHT, PANEL_WIDTH, PORTRAIT_SIZE, PanelBar,
    PanelText, UnitPanel, enemy_column, enemy_row, slot_prefix, status_bar, unit_panel,
};
pub use system::update_unit_panels_system;
