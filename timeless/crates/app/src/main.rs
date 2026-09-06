use bevy::prelude::*;
use bevy_ufbx::FbxPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(FbxPlugin)
        .add_systems(Startup, (app::preload, app::setup).chain())
        .add_systems(Update, system)
        .run();
}
fn system(mut gizmos: Gizmos) {
    // 创建一个变换：绕 X 轴旋转 -90 度，使得原本的 XZ 平面变为 XY 平面
    // 并将位置移动到平台表面 (y = -0.01)
    let transform = Isometry3d::new(
        Vec3::new(5.0 / 2.0, -0.01, 5.0 / 2.0),              // 平移
        Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2), // 旋转
    );

    gizmos
        .grid(
            transform,
            UVec2::new(5, 5),           // 网格数量 10x10
            Vec2::splat(1.0),           // 格子大小 2.0
            Color::srgb(0.0, 1.0, 0.0), // 绿色
        )
        .outer_edges(); // 可选：外边框更亮

    // 在世界原点绘制长度为 2.0 的坐标轴
    gizmos.axes(Transform::default(), 2.0);
}
