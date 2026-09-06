use bevy::prelude::*;

pub fn test() -> impl Scene {
    let transform = Transform::from_xyz(7.0, 0.5, 7.0).with_scale(Vec3::splat(3.0));
    bsn! {
        template_value(transform)
        WorldAssetRoot("models/nature/rock_largeA.glb#Scene0")
    }
}
