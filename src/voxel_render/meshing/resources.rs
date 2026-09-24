//! 网格化配置。

use bevy::prelude::*;

/// 网格化配置。
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshingConfig {
    /// 是否**贪婪网格化**（同类型共面的面合并成矩形）。
    ///
    /// `true`（默认）= 贪婪合并 + 面剔除：一个 32³ 的实心区块从 6144 个四边形
    /// 降到 6 个（默认地形实测）；`false` = 逐面输出**且不剔除**隐藏面，
    /// 保留原几何——只用于对照与调试（有测试把两条路的面积对上）。
    pub cull_hidden_faces: bool,
}

impl Default for MeshingConfig {
    fn default() -> Self {
        Self {
            cull_hidden_faces: true,
        }
    }
}
