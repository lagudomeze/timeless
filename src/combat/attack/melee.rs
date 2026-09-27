//! 近战横扫：数值与节奏的来源。
//!
//! ⚠️ **视觉不在这里**：`melee_scene` 搬去了 [`super::scene`]（那里是唯一会用
//! `asset_value(...)` 造网格 / 材质的地方，见 `docs/backlog/dev.md`）。

/// 近战横扫的伤害与属性（注册表 / HUD 展示也读这里，避免两处各写一份）。
pub const MELEE_DAMAGE: i32 = 15;

/// 从配置取近战伤害（缺省 = 常量）。
///
/// 与 `config.fireball.power` 同源：配置是**覆盖**，各域常量仍是"没有配置文件时"
/// 的行为（见 [`crate::config`]）。
pub fn melee_damage(config: Option<&crate::config::ActionConfig>) -> i32 {
    config
        .map(|config| config.melee.power)
        .unwrap_or(MELEE_DAMAGE)
}

/// 速度帧（信息层读数）。
pub const MELEE_FRAME: u32 = 5;
/// 打断力度：命中时和目标的打断抗性掷骰对抗。
pub const MELEE_POWER: i32 = 3;
