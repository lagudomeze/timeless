//! 开局组装：按场景工厂把初始世界拼出来（Startup 一次）。

use bevy::prelude::*;

use crate::presentation::{camera, decoration};
use crate::world::{TerrainConfig, ground_position};

use super::enemy::enemy_scene;
use super::player::player_scene;

/// 组装静态场景：灯光、相机、玩家、敌人、地表装饰。
///
/// 单位与装饰的落脚高度一律取自 [`crate::world`] 的地表函数，
/// 组装层不自己发明地形数据。
pub fn setup_scene(
    mut commands: Commands,
    natures: Res<decoration::Natures>,
    terrain: Res<TerrainConfig>,
) {
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, 0.8, 0.0)),
    ));

    commands.spawn_scene(camera::main_camera());
    commands.spawn_scene(camera::light());
    commands.spawn_scene(player_scene(&terrain));
    commands.spawn_scene(enemy_scene(&terrain));

    // 地表装饰：铺满可视范围，位置随地形起伏
    for x in 0..5 {
        for z in 0..5 {
            let position = ground_position(&terrain, 3.0 + x as f32 * 6.0, 3.0 + z as f32 * 6.0);
            commands.spawn_scene(natures.random(Transform::from_translation(position)));
        }
    }
}
