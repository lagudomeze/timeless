//! 箭矢：数值 + 通用射弹数据的来源。
//!
//! ⚠️ **视觉不在这里**：`arrow_scene` 搬去了 [`super::scene`]（那里是唯一会用
//! `asset_value(...)` 造网格 / 材质的地方，见 `docs/backlog/dev.md`）。

/// 箭矢的速度帧（**信息层读数**；越小越快——箭最快）。
pub const ARROW_FRAME: u32 = 4;
/// 箭矢伤害（**单体**：与火球的 AoE 12 点相对——单体更高，这是两种投射物的分工）。
pub const ARROW_DAMAGE: i32 = 10;

/// 从配置取箭矢伤害（缺省 = 常量）。
pub fn arrow_damage(config: Option<&crate::config::ActionConfig>) -> i32 {
    config
        .map(|config| config.shoot.power)
        .unwrap_or(ARROW_DAMAGE)
}

/// 箭矢花多少**弹药**：单体、快，所以比重击（火球）便宜。
pub const ARROW_COST: u32 = 1;
/// 箭矢的打断力度：轻，但快（见 `ARROW_FRAME`）。
pub const ARROW_POWER: i32 = 1;

/// 箭速（世界单位 / 秒）：执行器用它算「射手要忙到什么时候」。
pub const ARROW_SPEED: f32 = 12.0;
