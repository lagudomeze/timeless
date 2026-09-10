//! 体素明暗。
//!
//! v0.1 只做「面朝向明暗」：把系数烘焙进网格顶点色（材质基色 × 顶点色），
//! 让方块棱角在斜视角下看得出来。
//!
//! 真正的环境光遮蔽（AO）需要按顶点统计邻域遮挡体素，属于下一步优化；
//! 届时替换本模块即可，网格化与材质都不用动。

use bevy::prelude::*;

/// 面朝向 → 亮度系数（0..=1）。
///
/// 顶面最亮、底面最暗，X 侧比 Z 侧略暗——固定方向的假想光照，
/// 与场景里的 `DirectionalLight` 打光方向一致即可读出体积感。
pub fn face_shade(normal: IVec3) -> f32 {
    match (normal.x, normal.y, normal.z) {
        (_, 1, _) => 1.0,
        (_, -1, _) => 0.45,
        (0, 0, _) => 0.82,
        (_, 0, 0) => 0.66,
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_is_brighter_than_sides_which_are_brighter_than_bottom() {
        let top = face_shade(IVec3::Y);
        let side_z = face_shade(IVec3::Z);
        let side_x = face_shade(IVec3::X);
        let bottom = face_shade(IVec3::NEG_Y);
        assert!(top > side_z && side_z > side_x && side_x > bottom);
        assert!((0.0..=1.0).contains(&top) && (0.0..=1.0).contains(&bottom));
    }
}
