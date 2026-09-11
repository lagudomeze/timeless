//! 动作节奏与格子尺度的常量表。
//!
//! 无回合模型下没有「回合」这个时间单位，节奏完全由**每个动作自带的前摇 + 后摇**决定
//! （见 [docs/design/timeline-turnless.md](../../../docs/design/timeline-turnless.md) 第 4 节）。
//! 数值先集中在这里硬编码，Phase 2.1 再外置成 `.ron`。

/// 一格的世界边长（世界单位 / 格）。
///
/// 决策与同格判定按格算，命中 / 射程 / 爆炸按**真实距离**算，两者靠它换算。
/// 不取 1.0 是因为体素是 1×1×1，格子与体素一一对应会把单位压成 1 米大小，
/// 与现有模型缩放和视觉尺度对不上。
pub const CELL_SIZE: f32 = 2.0;

/// 单个动作的固定节奏：前摇（声明 → 执行）+ 后摇（执行 → 重新可决策）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActionTiming {
    /// 前摇（虚拟秒）：声明时刻 + 前摇 = 执行时刻。
    pub windup: f32,
    /// 后摇（虚拟秒）：执行时刻 + 后摇 = 重新可决策时刻。
    pub recovery: f32,
}

impl ActionTiming {
    /// 常量构造（`const` 便于在下方表格里直接写）。
    pub const fn new(windup: f32, recovery: f32) -> Self {
        Self { windup, recovery }
    }

    /// 从声明到重新可决策的总时长。
    pub fn total(&self) -> f32 {
        self.windup + self.recovery
    }
}

/// 移动：一格一步。
pub const MOVE: ActionTiming = ActionTiming::new(0.15, 0.10);
/// 跳跃：前摇最短，落地即恢复。
/// 跳跃：前摇最短；后摇覆盖整个弹道（约 0.6s），落地即可再决策。
pub const JUMP: ActionTiming = ActionTiming::new(0.10, 0.60);
/// 近战：出手快、硬直长。
pub const MELEE: ActionTiming = ActionTiming::new(0.20, 0.35);
/// 火球：出手慢、后摇长、威力大。
pub const SHOOT: ActionTiming = ActionTiming::new(0.30, 0.50);
/// 翻滚：防御性动作，几乎立即生效。
pub const ROLL: ActionTiming = ActionTiming::new(0.05, 0.30);
/// 招架：同上。
pub const PARRY: ActionTiming = ActionTiming::new(0.05, 0.25);
