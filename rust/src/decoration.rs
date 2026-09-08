use bevy::prelude::*;

pub struct Nature {
    pub path: &'static str,
    pub mix_scale: f32,
    pub max_scale: f32,
}

#[derive(Resource)]
pub struct Natures {
    gltf_list: Vec<Nature>,
}

impl Natures {
    pub fn random(&self, transform: Transform) -> impl Scene {
        let index = rand::random_range(0..self.gltf_list.len());
        let natuew_gltf = &self.gltf_list[index];
        let path = natuew_gltf.path;

        let scale = rand::random_range(natuew_gltf.mix_scale..natuew_gltf.max_scale);
        let transform = transform.with_scale(Vec3::splat(scale));
        bsn! {
            WorldAssetRoot(path)
            template_value(transform)
        }
    }
}

pub fn load_natures(commands: &mut Commands) {
    let mut gltf_list = Vec::new();

    //todo make it config
    for (path, mix_scale, max_scale) in [
        ("models/nature/tree_default.glb#Scene0", 1.2, 1.7),
        ("models/nature/tree_default_dark.glb#Scene0", 1.2, 1.7),
        ("models/nature/tree_oak.glb#Scene0", 1.3, 1.8),
        ("models/nature/tree_small.glb#Scene0", 0.9, 1.3),
        ("models/nature/tree_pineRoundA.glb#Scene0", 1.0, 1.5),
        ("models/nature/tree_pineTallA.glb#Scene0", 1.1, 1.5),
        ("models/nature/tree_palm.glb#Scene0", 1.1, 1.5),
        ("models/nature/rock_largeA.glb#Scene0", 0.5, 0.9),
        ("models/nature/rock_largeB.glb#Scene0", 0.5, 0.9),
        ("models/nature/rock_smallA.glb#Scene0", 0.4, 0.7),
        ("models/nature/stone_smallA.glb#Scene0", 0.4, 0.7),
        ("models/nature/plant_bush.glb#Scene0", 0.7, 1.1),
        ("models/nature/plant_bushSmall.glb#Scene0", 0.5, 0.8),
        ("models/nature/flower_redA.glb#Scene0", 0.6, 0.9),
        ("models/nature/flower_yellowA.glb#Scene0", 0.6, 0.9),
        ("models/nature/grass.glb#Scene0", 0.7, 1.0),
        ("models/nature/log.glb#Scene0", 0.8, 1.1),
        ("models/nature/stump_round.glb#Scene0", 0.8, 1.1),
    ] {
        gltf_list.push(Nature {
            path,
            mix_scale,
            max_scale,
        });
    }

    commands.insert_resource(Natures { gltf_list });
}
