use bevy::prelude::*;
use bevy_ufbx::FbxPlugin;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_plugins(FbxPlugin)
        .add_systems(Startup, (app::preload, app::setup).chain())
        .add_systems(Update, system);
    app::add_combat(&mut app);·
    app.run();
}
fn system(mut gizmos: Gizmos) {
    // 在世界原点绘制长度为 2.0 的坐标轴
    gizmos.axes(Transform::default(), 5.0);
}
