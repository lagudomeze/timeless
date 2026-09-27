//! 交互域的**场景工厂**：悬停高亮方块 + 两个预演指示器（都默认隐藏）。
//!
//! ## 为什么单独一个文件
//!
//! 这里是交互域**唯一**会用 `asset_value(...)` 现场造网格与材质的地方。
//! 留在 [`super::visual`] 里时，那个文件会同时回答"这一格该亮什么颜色"
//! （每帧的更新逻辑）与"这个方块长什么样"（一次性建实体），
//! 而只有后者需要渲染类型（见 `docs/backlog/dev.md`）。
//!
//! 分法照抄 [`equipment`](crate::equipment) 与
//! [`combat::attack`](crate::combat::attack)：**建实体归 `scene.rs`，
//! 每帧刷状态归 `visual.rs`**。
//!
//! 三样东西回答的是不同的问题（别混）：
//!
//! | 实体 | 回答 |
//! | :--- | :--- |
//! | [`HoverHighlight`] | 我**指着**哪一格 |
//! | [`AoePreview`] | 火球这一手**会打到哪**（圆盘） |
//! | [`ConePreview`] | 横扫这一手**会扫到哪**（扇形） |
//!
//! 它们都**不是**单位的子实体：位置每帧由 `visual.rs` 按格心算出来。

use bevy::prelude::*;
// 指示用薄片不该投影：它只是"我指着这里"，投出影子反而像实体
use bevy::light::NotShadowCaster;

use crate::combat::attack::{FIREBALL_RADIUS, MELEE_REACH};
use crate::movement::CELL_SIZE;

use super::components::{AoePreview, ConePreview, HoverHighlight, HoverTint};

/// 高亮方块离地高度（世界单位）：贴着地但不和地面 z-fighting。
pub const HIGHLIGHT_LIFT: f32 = 0.03;
/// 高亮方块边长占一格的比例（留一点缝，看得出格与格的边界）。
pub const HIGHLIGHT_FILL: f32 = 0.92;

/// 空地：青。
pub const HOVER_GROUND: Color = Color::srgba(0.35, 0.95, 0.95, 0.35);
/// 自己脚下：蓝（= 玩家色）。
pub const HOVER_PLAYER: Color = Color::srgba(0.35, 0.55, 0.95, 0.35);
/// 敌人脚下：红（= 敌人色）。
pub const HOVER_ENEMY: Color = Color::srgba(0.95, 0.35, 0.35, 0.40);

/// 火球 AOE 预演色（红，半透明：这是"会伤到谁"的范围）。
pub const AOE_PREVIEW: Color = Color::srgba(0.95, 0.30, 0.25, 0.22);
/// 近战扇形预演色（琥珀，和技能栏选中色同族）。
pub const CONE_PREVIEW: Color = Color::srgba(0.95, 0.84, 0.42, 0.22);
/// 近战扇形的张角（弧度）：正面 120°。
pub const CONE_ARC: f32 = std::f32::consts::TAU / 3.0;

/// 开局生成唯一的高亮方块（之后只搬位置 / 改颜色）。
pub fn spawn_hover_highlight(mut commands: Commands) {
    commands.spawn_scene(bsn! {
        Name("HoverHighlight")
        HoverHighlight
        HoverTint(Color::NONE)
        Mesh3d(asset_value(Rectangle::new(
            CELL_SIZE * HIGHLIGHT_FILL,
            CELL_SIZE * HIGHLIGHT_FILL,
        )))
        MeshMaterial3d<StandardMaterial>(asset_value(StandardMaterial {
            base_color: Color::NONE,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            ..default()
        }))
        // 平铺在地面上（和单位阴影同一套做法）
        Transform {
            rotation: {Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)},
        }
        Visibility::Hidden
        NotShadowCaster
    });
}

/// 开局生成两个预演指示器：火球 AOE 圆盘 + 近战扇形（都默认隐藏）。
///
/// 它们和悬停高亮是两回事：高亮回答"我指着哪一格"，预演回答"**这一手会打到哪**"。
pub fn spawn_preview_indicators(mut commands: Commands) {
    let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    commands.spawn_scene(bsn! {
        Name("AoePreview")
        AoePreview
        Mesh3d(asset_value(Circle::new(FIREBALL_RADIUS)))
        MeshMaterial3d<StandardMaterial>(asset_value(StandardMaterial {
            base_color: AOE_PREVIEW,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            ..default()
        }))
        Transform {
            rotation: {flat},
        }
        Visibility::Hidden
        NotShadowCaster
    });
    commands.spawn_scene(bsn! {
        Name("ConePreview")
        ConePreview
        // 扇形从局部 +X 轴张开；下面按"朝向悬停格"整体旋转
        Mesh3d(asset_value(CircularSector::new(MELEE_REACH, CONE_ARC)))
        MeshMaterial3d<StandardMaterial>(asset_value(StandardMaterial {
            base_color: CONE_PREVIEW,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            ..default()
        }))
        Transform {
            rotation: {flat},
        }
        Visibility::Hidden
        NotShadowCaster
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::scene::ScenePlugin;

    /// 场景工厂验收：BSN 建出来的实体必须**真的带上**标记组件。
    ///
    /// 这条守着一次真实的坑——`bsn!` 改成补丁式写法后，少写一行 `HoverHighlight`
    /// 或 `Visibility::Hidden` 不会编译报错，只会让高亮**永远显示 / 永远不显示**。
    /// 所以这里不看"长得对不对"，只钉死"标记在不在、默认藏没藏"。
    #[test]
    fn the_scene_factories_attach_their_markers() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins((AssetPlugin::default(), ScenePlugin))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>();
        app.add_systems(Startup, (spawn_hover_highlight, spawn_preview_indicators));
        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<(Entity, &Visibility), With<HoverHighlight>>();
        let highlights: Vec<(Entity, Visibility)> = query
            .iter(app.world())
            .map(|(entity, visibility)| (entity, *visibility))
            .collect();
        assert_eq!(highlights.len(), 1, "高亮方块应当只有一个");
        assert_eq!(
            highlights[0].1,
            Visibility::Hidden,
            "没悬停时就该藏着（少了 `Visibility::Hidden` 会整块糊在场上）"
        );

        // 两个预演指示器各一个，且都默认隐藏
        for (name, count) in [("AoePreview", 1), ("ConePreview", 1)] {
            let found = match name {
                "AoePreview" => app
                    .world_mut()
                    .query_filtered::<&Visibility, With<AoePreview>>()
                    .iter(app.world())
                    .count(),
                _ => app
                    .world_mut()
                    .query_filtered::<&Visibility, With<ConePreview>>()
                    .iter(app.world())
                    .count(),
            };
            assert_eq!(found, count, "{name} 应当由场景工厂建出来");
        }
        let mut hidden = app
            .world_mut()
            .query_filtered::<&Visibility, (With<AoePreview>, Without<HoverHighlight>)>();
        assert!(
            hidden.iter(app.world()).all(|v| *v == Visibility::Hidden),
            "预演指示器开局应当都是隐藏的"
        );
    }
}
