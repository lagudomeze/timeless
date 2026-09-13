//! 资源预载：环境光、装饰模型表与单位精灵（Startup）。

use bevy::prelude::*;

use super::decoration;
use super::unit_sprite;

/// 预载全局资源：环境光 + 装饰模型表 + 单位精灵贴图。
///
/// 场景组装（[`crate::spawn::setup_scene`]）依赖这里产出的 `Natures` 与
/// [`unit_sprite::UnitSprites`]，因此 `PreloadSet` 必须排在 `AssemblySet` 之前（顺序在
/// [`GamePlugin`](crate::GamePlugin) 里声明）。
pub fn preload(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 400.0,
        ..default()
    });
    decoration::load_natures(&mut commands);
    unit_sprite::load_unit_sprites(&mut commands, &assets);
}
