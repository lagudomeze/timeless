//! 屏幕 → 世界格：射线拾取（纯函数，可脱离渲染单测）。
//!
//! 采样的是**地形高度场**（[`surface_height_at`]），沿射线以 `PICK_STEP` 步进，
//! 第一次钻到地表以下就算命中。为什么不用体素 DDA：A 的世界现在就是高度场
//! （没有悬垂），步进不需要读 `ChunkMap`、不受区块加载影响，而且是纯函数——
//! 单测里手搓一条射线就能验，不需要相机 / 窗口。
//! 等有了悬垂 / 洞穴（体素碰撞那一步）再换 DDA，**接口不变**。

use bevy::prelude::*;

use crate::movement::CELL_SIZE;
use crate::movement::Cell;
use crate::world::{TerrainConfig, terrain::surface_height_at};

/// 射线最长探测距离（世界单位）：超出就当没指到地面。
pub const MAX_PICK_DISTANCE: f32 = 160.0;
/// 步长：1/4 格。地形起伏 ≤ 1 格，这个步长不会跨过山脊漏格。
pub const PICK_STEP: f32 = CELL_SIZE * 0.25;

/// 屏幕坐标（**逻辑像素**）→ 世界射线。
///
/// 相机是透视投影；视口与窗口等大时，`cursor` 与 `Window::cursor_position` 同一套坐标
/// （BRP 的 `move_mouse` 也是这套），不会因为 DPI 缩放产生偏差。
pub fn cursor_ray(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    cursor: Vec2,
) -> Option<Ray3d> {
    camera.viewport_to_world(camera_transform, cursor).ok()
}

/// 射线 → 命中的格；指到天空 / 超出探测距离返回 `None`。
pub fn pick_cell(ray: Ray3d, terrain: &TerrainConfig) -> Option<Cell> {
    let mut travelled = 0.0;
    while travelled <= MAX_PICK_DISTANCE {
        let point = ray.get_point(travelled);
        if point.y <= surface_height_at(terrain, point.x, point.z) as f32 {
            return Some(Cell::from_world(point));
        }
        travelled += PICK_STEP;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terrain() -> TerrainConfig {
        TerrainConfig::default()
    }

    /// 从高处垂直向下：落在正下方那一格。
    #[test]
    fn a_downward_ray_hits_the_cell_under_it() {
        let terrain = terrain();
        let cell = Cell::new(2, 3);
        let center = cell.center();
        let ray = Ray3d::new(Vec3::new(center.x, 20.0, center.y), Dir3::NEG_Y);

        assert_eq!(pick_cell(ray, &terrain), Some(cell));
    }

    /// 朝天空的射线不命中。
    #[test]
    fn a_ray_into_the_sky_misses() {
        let terrain = terrain();
        let ray = Ray3d::new(Vec3::new(1.0, 1.0, 1.0), Dir3::Y);

        assert_eq!(pick_cell(ray, &terrain), None);
    }

    /// 斜射：命中脚下这一格，而不是穿过去打到更远的地方。
    #[test]
    fn a_steep_ray_stops_at_the_first_cell_it_hits() {
        let terrain = terrain();
        let cell = Cell::new(1, 1);
        let center = cell.center();
        // 从格中心斜上方 6 格外，45° 朝下指回格中心
        let origin = Vec3::new(center.x + 6.0, 6.0, center.y + 6.0);
        let direction =
            Dir3::new((Vec3::new(center.x, 0.0, center.y) - origin).normalize()).unwrap();
        let ray = Ray3d::new(origin, direction);

        let hit = pick_cell(ray, &terrain).expect("斜射线应当命中地面");
        assert!(
            hit == cell || (hit.x - cell.x).abs() <= 1 && (hit.z - cell.z).abs() <= 1,
            "斜射的落点应当在目标格附近，实际 {hit:?}"
        );
    }

    /// 水平射线打不到地面：不能无限步进，必须按时返回。
    #[test]
    fn a_horizontal_ray_gives_up_after_the_max_distance() {
        let terrain = terrain();
        let ray = Ray3d::new(Vec3::new(0.0, 100.0, 0.0), Dir3::X);

        assert_eq!(
            pick_cell(ray, &terrain),
            None,
            "超出探测距离就该放弃，而不是一直步进下去"
        );
    }
}
