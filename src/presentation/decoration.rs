//! 地表装饰：glTF 模型表 + 随机摆放场景工厂。

use bevy::prelude::*;

/// 一种装饰模型及其缩放区间。
pub struct Nature {
    pub path: &'static str,
    pub min_scale: f32,
    pub max_scale: f32,
}

/// 装饰模型表（资源）。
#[derive(Resource)]
pub struct Natures {
    models: Vec<Nature>,
}

impl Natures {
    /// 随机挑一个模型、随机缩放，生成摆放场景。
    pub fn random(&self, transform: Transform) -> impl Scene {
        let index = rand::random_range(0..self.models.len());
        let nature = &self.models[index];
        let path = nature.path;
        let scale = rand::random_range(nature.min_scale..nature.max_scale);
        let transform = transform.with_scale(Vec3::splat(scale));
        bsn! {
            WorldAssetRoot(path)
            template_value(transform)
        }
    }
}

/// 预载装饰表（Startup 时插入资源）。
pub fn load_natures(commands: &mut Commands) {
    // TODO: 改成外部配置（见 TODO.md 的 Phase 2.1）
    let models = [
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
    ]
    .into_iter()
    .map(|(path, min_scale, max_scale)| Nature {
        path,
        min_scale,
        max_scale,
    })
    .collect();

    commands.insert_resource(Natures { models });
}
