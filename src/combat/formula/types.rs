//! 伤害类型。

/// 伤害类型：日志 / 抗性 / 特效的区分维度。
///
/// 目前只有物理伤害；新增元素伤害时只加变体与一个 formula 系统，
/// 目标获取、生命值、清理链路都不用改。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DamageType {
    #[default]
    Physical,
}

impl DamageType {
    /// 中文标签（战斗日志用）。
    pub fn label(self) -> &'static str {
        match self {
            Self::Physical => "物理",
        }
    }
}
