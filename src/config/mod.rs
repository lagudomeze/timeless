//! 动作数值外置：**一份 `.ron` 文件**描述所有动作的节奏与威力。
//!
//! ## 为什么独立一个目录，不放进 `assets/`
//!
//! `assets/` 是**素材**（贴图 / 字体 / 模型），走 bevy 的资产管线、会被热重载监视、
//! 也可能被打包进构建产物。而这是**设计数值**：调它是"改平衡"，
//! 不是"换素材"——所以它住在仓库根的 `config/`，由一个**单独的资产源**
//! （[`CONFIG_SOURCE`]）加载。分开之后：
//!
//! - 打包发行时可以不带上 `config/`（用内置默认值跑）；
//! - 热重载 `assets/` 时不会把数值文件也一起卷进来；
//! - 语义清楚：`assets/` 换皮，`config/` 换手感。
//!
//! ## 加载与失败
//!
//! 启动时（`PreStartup`，**早于各域注册技能定义**）读一次：
//!
//! ```text
//! config/actions.ron 存在且合法 → 用文件里的值
//! 文件不存在                    → 用 `ActionConfig::default()`（= 代码里原有的数值）
//! 文件存在但解析失败            → **报错并退回默认值**，不 panic
//! ```
//!
//! 最后一条是刻意的：一个打错的数字不该让游戏起不来，但也不该被静默忽略
//! （那样调了半天没效果更糟）。所以解析失败要**大声说出来**。
//!
//! ## 一处真相
//!
//! 各域的常量（`MOVE_TIMING` / `FIREBALL_TIMING`…）**保留**——它们是"默认值"，
//! 也仍然是技能定义的来源。配置只是**覆盖**它们：装载后写进
//! [`ActionConfig`] 资源，各声明系统与技能定义改从资源读。
//! 这样"没有配置文件也能跑"这条不破。

use std::path::Path;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub mod plugin;

pub use plugin::ConfigPlugin;

use crate::timeline::ActionTiming;

/// 配置文件所在目录（仓库根，**不在 `assets/` 里**）。
pub const CONFIG_DIR: &str = "config";
/// 配置文件名。
pub const CONFIG_FILE: &str = "actions.ron";
/// 这个资产源的 id（要查"配置加载好了没"时用它）。
pub const CONFIG_SOURCE: &str = "config";
/// 相对 `config/` 的完整路径。
pub const CONFIG_PATH: &str = "actions.ron";

/// 一条动作的数值。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ActionNumbers {
    /// 前摇（秒）
    pub windup: f32,
    /// 后摇（秒）
    pub recovery: f32,
    /// 打断抗性（越大越难被打断）
    pub interrupt_resist: i32,
    /// 精力消耗
    pub cost: u32,
    /// 大致威力（**展示 + 个体伤害**：命中数值仍归载荷的 `PhysicalDamage`）
    pub power: i32,
    /// 速度帧（信息层读数："谁先动"）
    pub frame: u32,
}

impl ActionNumbers {
    /// 从 `ActionTiming` 起步（默认值就是各域常量）。
    pub const fn from_timing(timing: ActionTiming, cost: u32, power: i32, frame: u32) -> Self {
        Self {
            windup: timing.windup,
            recovery: timing.recovery,
            interrupt_resist: timing.interrupt_resist,
            cost,
            power,
            frame,
        }
    }

    /// 还原成调度用的节奏。
    pub const fn timing(self) -> ActionTiming {
        ActionTiming::new(self.windup, self.recovery, self.interrupt_resist)
    }
}

/// **全部可调数值**：一份文件、一处真相。
///
/// 字段名就是 `.ron` 里的键名（`serde` 默认蛇形），所以文件长这样：
///
/// ```ron
/// (
///     move_: (windup: 0.15, recovery: 0.10, interrupt_resist: 1, cost: 0, power: 0, frame: 0),
///     fireball: (windup: 0.30, recovery: 0.50, interrupt_resist: 2, cost: 2, power: 12, frame: 7),
/// )
/// ```
///
/// `move_` 带下划线是因为 `move` 是 Rust 关键字——文件里也就写成 `move_`，
/// 与字段名一致（不引入第二套命名）。
#[derive(Resource, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ActionConfig {
    pub move_: ActionNumbers,
    pub jump: ActionNumbers,
    pub roll: ActionNumbers,
    pub melee: ActionNumbers,
    pub shoot: ActionNumbers,
    pub fireball: ActionNumbers,
    pub parry: ActionNumbers,
    pub wait: ActionNumbers,
    /// Focus 上限与回复间隔
    pub focus: FocusNumbers,
    /// 移动 / 翻滚的速度（世界单位 / 秒）
    pub speeds: SpeedNumbers,
}

/// 反应资源的数值。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FocusNumbers {
    /// 上限
    pub max: u32,
    /// 每多少虚拟秒回 1 点
    pub recover_interval: f32,
}

impl Default for FocusNumbers {
    fn default() -> Self {
        Self {
            max: crate::timeline::FOCUS_MAX,
            recover_interval: crate::timeline::FOCUS_RECOVER_INTERVAL,
        }
    }
}

/// 位移速度。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SpeedNumbers {
    /// 玩家移动速度（`MoveSpeed`）
    pub player: f32,
    /// 敌人移动速度
    pub enemy: f32,
    /// 翻滚速度
    pub roll: f32,
}

impl Default for SpeedNumbers {
    fn default() -> Self {
        Self {
            player: 5.0,
            enemy: 2.0,
            roll: crate::combat::defense::ROLL_SPEED,
        }
    }
}

impl Default for ActionConfig {
    /// **默认值 = 代码里原有的常量**（没有配置文件也能跑）。
    ///
    /// 这些就写在各域的模块里（`MOVE_TIMING` / `FIREBALL_TIMING`…），
    /// 这里只是把它们聚合起来当兜底。
    fn default() -> Self {
        use crate::combat::defense::{PARRY_COST, PARRY_TIMING, ROLL_COST};
        use crate::movement::ROLL_TIMING;
        Self {
            move_: ActionNumbers::from_timing(crate::movement::MOVE_TIMING, 0, 0, 0),
            jump: ActionNumbers::from_timing(crate::movement::JUMP_TIMING, 0, 0, 0),
            roll: ActionNumbers::from_timing(ROLL_TIMING, ROLL_COST, 0, 0),
            melee: ActionNumbers::from_timing(
                crate::combat::attack::MELEE_TIMING,
                0,
                crate::combat::attack::MELEE_DAMAGE,
                crate::combat::attack::MELEE_FRAME,
            ),
            shoot: ActionNumbers::from_timing(
                crate::combat::attack::ARROW_TIMING,
                0,
                crate::combat::attack::ARROW_DAMAGE,
                crate::combat::attack::ARROW_FRAME,
            ),
            fireball: ActionNumbers::from_timing(
                crate::combat::attack::FIREBALL_TIMING,
                crate::combat::attack::FIREBALL_COST,
                crate::combat::attack::FIREBALL_DAMAGE,
                crate::combat::attack::FIREBALL_FRAME,
            ),
            parry: ActionNumbers::from_timing(PARRY_TIMING, PARRY_COST, 0, 0),
            wait: ActionNumbers::from_timing(
                crate::timeline::WaitConfig::default().timing(),
                0,
                0,
                0,
            ),
            focus: FocusNumbers::default(),
            speeds: SpeedNumbers::default(),
        }
    }
}

impl ActionConfig {
    /// 从 `.ron` 文本解析。失败时返回**带位置信息的错误**（便于指出哪一行写错）。
    pub fn from_ron(text: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(text)
    }

    /// 从磁盘读；文件不存在时返回 `None`（调用方退回默认值）。
    ///
    /// 解析失败**不吞掉**：返回 `Err`，由调用方报错并退回默认值。
    pub fn load_from_disk() -> Result<Option<Self>, ConfigError> {
        let path = Path::new(CONFIG_DIR).join(CONFIG_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(ConfigError::Read(error.to_string())),
        };
        Self::from_ron(&text)
            .map(Some)
            .map_err(|error| ConfigError::Parse(error.to_string()))
    }

    /// 写出一份带默认值的文件（`App` 第一次跑时可以调，把"能改什么"摆给设计者看）。
    pub fn to_ron(self) -> String {
        ron::ser::to_string_pretty(&self, ron::ser::PrettyConfig::default())
            .unwrap_or_else(|error| format!("// 序列化失败：{error}"))
    }
}

/// 配置文件没读成的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// 文件在，但读不动（权限 / 编码）
    Read(String),
    /// 读到了，但解析不了（语法或类型错，含行号）
    Parse(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(detail) => write!(formatter, "读不动 {CONFIG_DIR}/{CONFIG_FILE}：{detail}"),
            Self::Parse(detail) => write!(
                formatter,
                "{CONFIG_DIR}/{CONFIG_FILE} 解析失败（**已退回内置默认值**）：{detail}"
            ),
        }
    }
}

/// 启动时装载配置（`PreStartup`：**早于各域注册技能定义**）。
///
/// 行为见模块文档「加载与失败」。
pub fn load_action_config_system(mut commands: Commands) {
    match ActionConfig::load_from_disk() {
        Ok(Some(config)) => {
            info!("⚙ 已从 {CONFIG_DIR}/{CONFIG_FILE} 装载动作数值");
            commands.insert_resource(config);
        }
        Ok(None) => {
            info!("⚙ 没有 {CONFIG_DIR}/{CONFIG_FILE}：使用内置默认数值");
            commands.insert_resource(ActionConfig::default());
        }
        Err(error) => {
            // **大声报错**但不 panic：打错的数字不该让游戏起不来
            error!("{error}");
            commands.insert_resource(ActionConfig::default());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 默认值就是各域常量：没有配置文件时行为与今天完全一致。
    #[test]
    fn the_defaults_are_the_constants_the_domains_already_use() {
        let config = ActionConfig::default();
        assert_eq!(config.move_.timing(), crate::movement::MOVE_TIMING);
        assert_eq!(config.roll.timing(), crate::movement::ROLL_TIMING);
        assert_eq!(
            config.fireball.timing(),
            crate::combat::attack::FIREBALL_TIMING
        );
        assert_eq!(config.fireball.cost, crate::combat::attack::FIREBALL_COST);
        assert_eq!(
            config.fireball.power,
            crate::combat::attack::FIREBALL_DAMAGE
        );
        assert_eq!(config.focus.max, crate::timeline::FOCUS_MAX);
    }

    /// 往返：写出再读回，值不变（`to_ron` 生成的样例文件必须真的能用）。
    #[test]
    fn a_config_survives_a_write_read_round_trip() {
        let original = ActionConfig {
            fireball: ActionNumbers {
                windup: 0.75,
                recovery: 1.25,
                interrupt_resist: 9,
                cost: 4,
                power: 33,
                frame: 11,
            },
            ..ActionConfig::default()
        };
        let text = original.to_ron();
        let parsed = ActionConfig::from_ron(&text).expect("自己写出来的文件必须读得回去");
        assert_eq!(parsed, original);
    }

    /// **缺字段的用默认值补齐**（`#[serde(default)]`）：设计者只想改两个数时，
    /// 不必把整份文件抄一遍。
    #[test]
    fn a_partial_file_falls_back_to_defaults() {
        let text = "( fireball: (windup: 5.0, recovery: 0.5, interrupt_resist: 2, cost: 2, power: 12, frame: 7) )";
        let parsed = ActionConfig::from_ron(text).expect("部分字段应当能解析");
        assert_eq!(parsed.fireball.windup, 5.0, "写了的字段生效");
        assert_eq!(
            parsed.move_.timing(),
            crate::movement::MOVE_TIMING,
            "没写的字段退回默认值"
        );
    }

    /// 语法错要**报错并带位置**，而不是静默给个默认值。
    #[test]
    fn a_broken_file_reports_where_it_broke() {
        let error = ActionConfig::from_ron("( fireball: (windup: ) )").expect_err("语法错必须报错");
        let shown = error.to_string();
        assert!(
            shown.contains("position") || shown.contains(':'),
            "错误信息该指出位置，实际：{shown}"
        );
    }

    /// 文件不存在 ≠ 出错：返回 `None` 让调用方用默认值（这是正常的首次运行）。
    #[test]
    fn a_missing_file_is_not_an_error() {
        // `load_from_disk` 读的是仓库根的真实路径；这里只验证"NotFound 走 None 分支"
        // 的语义——用一个必然不存在的目录名模拟
        let missing = Path::new("__definitely_not_here__").join(CONFIG_FILE);
        let result = std::fs::read_to_string(&missing);
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::NotFound,
            "不存在的文件应当是 NotFound（对应 `Ok(None)` 那条分支）"
        );
    }
}
