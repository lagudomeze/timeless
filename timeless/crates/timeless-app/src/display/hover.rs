//! # 悬停坐标显示：鼠标所在格子的坐标读数
//!
//! 把鼠标从屏幕坐标投影成世界射线，与地面平面（y≈0）求交得到世界坐标，
//! 再换算成网格坐标，显示在专用 UI 区域（屏幕右上角）。
//! 不在网格范围内时显示占位符「—」。

use bevy::prelude::*;

use crate::display::camera::MainCamera;
use crate::display::map::world_to_cell;

/// 悬停坐标文本标记（屏幕右上角专用区域）
#[derive(Component, Debug, Clone, Copy)]
pub struct HoverInfoText;

/// 创建悬停坐标 UI 文本（setup 调用一次）
pub fn spawn_hover_info(commands: &mut Commands, asset_server: &AssetServer) {
    let font = asset_server.load::<Font>("fonts/NotoSansSC-Regular.otf");
    commands.spawn((
        HoverInfoText,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            right: Val::Px(12.0),
            ..default()
        },
        Text::new("格坐标 —"),
        TextFont {
            font: FontSource::Handle(font),
            font_size: FontSize::Px(16.0),
            ..default()
        },
        // NoWrap：与 HUD 一致，绕开 ICU4X 缺失的 CJK 分词模型（避免报错刷屏）
        TextLayout {
            linebreak: LineBreak::NoWrap,
            justify: Justify::Left,
        },
        TextColor(Color::WHITE),
    ));
}

/// 悬停坐标：每帧投影鼠标射线并更新文本（仅内容变化时写回，避免无谓重排版）
pub fn hover_info_system(
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    window: Single<&Window>,
    mut text_q: Query<&mut Text, With<HoverInfoText>>,
) {
    let Ok(mut text) = text_q.single_mut() else {
        return;
    };
    let (camera, cam_tf) = *camera;

    let label = hovered_cell(camera, cam_tf, &window)
        .map(|cell| format!("格坐标 ({}, {})", cell.x, cell.y))
        .unwrap_or_else(|| "格坐标 —".to_string());
    if text.0.as_str() != label {
        **text = label;
    }
}

/// 鼠标 → 世界射线 → 地面交点 → 网格坐标（越界 / 无光标 / 射线平行地面时返回 `None`）
fn hovered_cell(camera: &Camera, cam_tf: &GlobalTransform, window: &Window) -> Option<IVec2> {
    let cursor = window.cursor_position()?;
    let ray = camera.viewport_to_world(cam_tf, cursor).ok()?;
    if ray.direction.y.abs() < f32::EPSILON {
        return None; // 视线平行于地面，无交点
    }
    let t = -ray.origin.y / ray.direction.y;
    if t <= 0.0 {
        return None; // 交点在相机背后
    }
    let hit = ray.origin + ray.direction * t;
    world_to_cell(hit)
}
