use bevy::prelude::*;

mod camera;
mod character;
pub mod combat;
mod decoration;
pub mod domain;
mod map;
mod movement;

pub fn preload(mut commands: Commands) {
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 400.0,
        ..default()
    });
    decoration::load_natures(&mut commands);
}

pub fn setup(mut commands: Commands, natures: Res<decoration::Natures>) {
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, 0.8, 0.0)),
    ));

    commands.spawn_scene(camera::main_camera());
    commands.spawn_scene(camera::light());
    commands.spawn_scene(bsn! {
        @map::BatteMap {
            @width : 10.0,
            @height : 10.0,
        }
    });
    commands.spawn_scene(character::test());

    for x in 0..5 {
        for y in 0..5 {
            let transform = Transform::from_xyz(x as f32, 0.0, y as f32);
            commands.spawn_scene(natures.random(transform));
        }
    }
}
