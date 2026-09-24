//! 体素明暗：**面朝向明暗 + 顶点环境光遮蔽（AO）**。
//!
//! 两者都烘焙进网格顶点色（材质基色 × 顶点色），所以材质与渲染管线都不用知道它们：
//!
//! - [`face_shade`]：一整面的朝向系数（顶面最亮、底面最暗）——方块棱角在斜视角下
//!   看得出来；
//! - [`vertex_occlusion`]：**逐顶点**的遮蔽，靠"这个角贴着几个实心邻居"算出来——
//!   凹角自然变暗，立方体因此不只是一块平色。
//!
//! ## 为什么 AO 是逐顶点而不是逐面
//!
//! 逐面只能给出"这一整面多亮"，而一个面上的四个角受的遮挡**可以完全不同**
//! （比如墙角的面：靠内那个角被两侧夹住、外角敞开）。AO 的可见效果正是这层
//! 由暗到亮的过渡，逐面做不出来。
//!
//! ## 与贪婪网格化的关系
//!
//! AO 值**必须进合并键**：两个共面的格子只有在类型与四个角的 AO 都相同时才能并成
//! 一个矩形——否则并出来的方块会拿一个角的亮度涂满整片（凹角变亮、平面变脏）。
//! 这条由 `meshing` 的合并判据保证，代价是"起伏处少并几块"，平坦处照旧全并。

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

/// 遮蔽最重时的亮度（环境光下限）：**不归零**——全黑会让凹角看起来像洞，
/// 而真实环境里再深的角落也有漫反射光。
pub const MIN_AO_SHADE: f32 = 0.55;

/// 顶点遮挡**等级**（`0` = 最暗，`3` = 全亮）。
///
/// 判据是体素 AO 的教科书式子（"那个角被几个邻居挡住"）：
///
/// ```text
/// 两侧都实心 → 0（那个角被两面夹住，斜角再实心也没用）
/// 否则 → 3 − 实心邻居数（0..3）
/// ```
///
/// **返回整数而不是亮度**：贪婪网格化要把"这个角的 AO"放进**合并键**，
/// 而浮点相等在键里既脆又难读。整数等级是精确的、可比较的，
/// 亮度再由 [`shade_of_level`] 一次性换算——**判定与换算只有这一处**。
///
/// 参数是**布尔**而不是查表：查体素是调用方（网格化）的事，这里只做判定，
/// 因此可以脱离区块直接单测（本文件的测试就是这么做的）。
pub fn occlusion_level(side1: bool, side2: bool, corner: bool) -> u8 {
    if side1 && side2 {
        0
    } else {
        3 - (side1 as u8 + side2 as u8 + corner as u8)
    }
}

/// 遮挡等级 → 亮度系数（[`MIN_AO_SHADE`] ..= 1.0）。
pub fn shade_of_level(level: u8) -> f32 {
    let level = level.min(3) as f32;
    MIN_AO_SHADE + (1.0 - MIN_AO_SHADE) * (level / 3.0)
}

/// 顶点环境光遮蔽：一个顶点贴着 `side1` / `side2` / `corner` 三个方向的实心邻居时，
/// 返回它的亮度系数（[`MIN_AO_SHADE`] ..= 1.0）。
///
/// 它只是 [`occlusion_level`] + [`shade_of_level`] 的组合——留这个方法是因为
/// 读代码时"这里是亮度"比"这里是等级"更直接。
pub fn vertex_occlusion(side1: bool, side2: bool, corner: bool) -> f32 {
    shade_of_level(occlusion_level(side1, side2, corner))
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

    /// 遮挡越多越暗，且**永远不归零**（全黑会让凹角看起来像洞）。
    #[test]
    fn more_occlusion_is_darker_but_never_black() {
        let open = vertex_occlusion(false, false, false);
        let one_side = vertex_occlusion(true, false, false);
        let both_sides = vertex_occlusion(true, true, false);

        assert_eq!(open, 1.0, "没有被挡住就是全亮");
        assert!(one_side < open, "挡住一侧要变暗");
        assert!(both_sides < one_side, "挡住两侧更暗");
        assert!(both_sides >= MIN_AO_SHADE, "最暗也有下限，不归零");
        const { assert!(MIN_AO_SHADE > 0.0 && MIN_AO_SHADE < 1.0, "下限要落在中间") };
    }

    /// **两侧都实心时斜角不再加分**（它已经被夹住了）。
    #[test]
    fn a_corner_already_pinched_on_both_sides_ignores_the_diagonal() {
        assert_eq!(
            vertex_occlusion(true, true, false),
            vertex_occlusion(true, true, true),
            "两面夹住时，斜角的实心与否不改变结果"
        );
    }

    /// 单调性：任意一个方向的遮挡只能变暗、不能变亮。
    #[test]
    fn adding_any_occluder_never_brightens_the_vertex() {
        for bits in 0..8u8 {
            let (s1, s2, c) = (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0);
            let base = vertex_occlusion(s1, s2, c);
            if !s1 {
                assert!(vertex_occlusion(true, s2, c) <= base, "加一侧不该变亮");
            }
            if !c && !(s1 && s2) {
                assert!(
                    vertex_occlusion(s1, s2, true) <= base,
                    "加斜角不该变亮（被两面夹住时则是相等）"
                );
            }
        }
    }
}
