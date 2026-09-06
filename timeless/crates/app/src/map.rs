use bevy::prelude::*;

#[derive(SceneComponent, Default, Clone)]
#[scene(BattleMapParams)]
pub struct BatteMap;

pub struct BattleMapParams {
    // x, y
    pub width: f32,
    pub height: f32,
    pub color: Color,
}

impl Default for BattleMapParams {
    fn default() -> Self {
        Self {
            width: 30.0,
            height: 30.0,
            color: Color::srgb(0.08, 0.10, 0.12),
        }
    }
}

impl BatteMap {
    fn scene(params: BattleMapParams) -> impl Scene {
        bsn! {
            Mesh3d(asset_value(Plane3d::default()
                .mesh()
                .size(params.width, params.height)))
            MeshMaterial3d<StandardMaterial>(asset_value( StandardMaterial {
                base_color: params.color,
                ..default()
            }))
            Transform::from_xyz(params.width / 2.0, -0.01, params.height / 2.0)
        }
    }
}
