//! 网格化配置。

use bevy::prelude::*;

/// 网格化配置。
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshingConfig {
    /// 是否剔除被实心邻居挡住的隐藏面。
    ///
    /// v0.1 的优化手段就是「面剔除」：每个方块只画暴露在外的面。
    /// 贪婪网格化（合并同材质共面为矩形）在数据量变大后再做。
    pub cull_hidden_faces: bool,
}

impl Default for MeshingConfig {
    fn default() -> Self {
        Self {
            cull_hidden_faces: true,
        }
    }
}
