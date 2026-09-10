//! 地形生成参数（Resource）。

use bevy::prelude::*;

/// 地形生成配置。
///
/// 同一份配置 + 同一个世界坐标永远推出同一高度，因此区块之间自然无缝，
/// 也不需要保存「原始地形」——只有玩家改动过的数据才值得持久化。
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct TerrainConfig {
    /// 噪声种子（换种子 = 换世界）
    pub seed: u32,
    /// 地表基准高度（世界体素 y）：地表最高不超过它
    pub base_height: i32,
    /// 起伏幅度：地表高度落在 `base_height - amplitude ..= base_height`
    pub amplitude: i32,
    /// 噪声采样步长（体素）：越小起伏越碎
    pub scale: f32,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            seed: 0x5EED,
            base_height: 0,
            // 起伏克制一点：单位目前不会跟着地形爬坡（体素碰撞属于后续项）
            amplitude: 1,
            scale: 6.0,
        }
    }
}
