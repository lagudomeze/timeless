//! 相机与方向光：场景零件 + 中键拖拽平移 + 滚轮缩放 + 跟随玩家。
//!
//! **机位的数学在 [`super::components`] 的 [`CameraRig`] 里**（纯函数、可单测）；
//! 本模块只做"读输入 → 调 `CameraRig` 的方法 → 刷新 `Transform`"。
//!
//! 输入域只把鼠标位移 / 滚轮翻译成 [`PanCamera`] / [`ZoomCamera`] 消息，
//! 本模块消费它们并改相机机位——游戏状态（单位、时间线）一点没碰。

use bevy::prelude::*;

use crate::combat::Faction;

use super::components::{CameraRig, MainCamera};

/// 相机平移请求（写：[`crate::input`]；消费：[`camera_pan_system`]）。
///
/// `delta` 是鼠标位移（像素），语义是「抓住地面拖动」。
#[derive(Message, Debug, Clone, Copy)]
pub struct PanCamera {
    pub delta: Vec2,
}

/// 滚轮缩放请求（写：[`crate::input`]；消费：[`camera_zoom_system`]）。
///
/// `delta` 是"滚轮步数"：正 = 拉远（机位退后抬高），负 = 拉近。
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct ZoomCamera {
    pub delta: f32,
}

/// 一格滚轮 = 多少缩放量。
const ZOOM_PER_NOTCH: f32 = 0.12;

/// 每像素平移的世界距离。
const PAN_PER_PIXEL: f32 = 0.05;

/// 主相机：斜视角看向玩家与敌人的中间。
pub fn main_camera() -> impl Scene {
    let rig = CameraRig::new(Vec3::new(4.0, 0.0, 4.0));
    let transform = rig.transform();
    bsn! {
        MainCamera
        Camera3d
        IsDefaultUiCamera
        template_value(rig)
        template_value(transform)
    }
}

/// 中键拖拽 → 拉出**观察偏移**：往哪边拖，地面就往哪边走（grab-drag 手感）。
///
/// 换算细节在 [`CameraRig::drag_ground`]；改的是观察偏移而不是 `focus`，
/// 真正的机位由 [`camera_follow_system`] 拉向「PC + 偏移」——松手后镜头不会弹回，
/// PC 也不会被甩出画面。
pub fn camera_pan_system(
    mut requests: MessageReader<PanCamera>,
    // 只读机位朝向、只改观察偏移：机位位置交给 `camera_follow_system`
    mut rigs: Query<(&mut CameraRig, &Transform)>,
) {
    let mut delta = Vec2::ZERO;
    for request in requests.read() {
        delta += request.delta;
    }
    if delta == Vec2::ZERO {
        return;
    }

    for (mut rig, transform) in &mut rigs {
        rig.drag_ground(transform.rotation, delta, PAN_PER_PIXEL);
    }
}

/// 滚轮缩放：把请求累加后按每格 [`ZOOM_PER_NOTCH`] 改机位倍率（上下限在 [`CameraRig`]）。
pub fn camera_zoom_system(
    mut requests: MessageReader<ZoomCamera>,
    mut rigs: Query<(&mut CameraRig, &mut Transform)>,
) {
    let mut notches = 0.0;
    for request in requests.read() {
        notches += request.delta;
    }
    if notches == 0.0 {
        return;
    }
    for (mut rig, mut transform) in &mut rigs {
        rig.apply_zoom(notches * ZOOM_PER_NOTCH);
        *transform = rig.transform();
    }
}

/// 镜头跟随玩家：`focus` 收敛到「玩家脚下 + 观察偏移」，让 PC 留在画面中心。
///
/// - 用 `Time<Real>` 而不是虚拟时间：世界冻结（等玩家输入）时镜头也该把上一段移动
///   追完，否则会僵在半路；
/// - 第一帧**直接吸附**（[`CameraRig::follow_blend`]），避免开局从初始机位慢慢飘过去；
/// - 玩家不存在（死亡 / 重置的中间帧）就保持原位。
pub fn camera_follow_system(
    time: Res<Time<Real>>,
    // 两个查询都碰 `Transform`，必须显式声明互斥（B0001）：
    // 单位不带 `CameraRig`，机位不带 `Faction`
    players: Query<(&Transform, &Faction), Without<CameraRig>>,
    mut rigs: Query<(&mut CameraRig, &mut Transform), Without<Faction>>,
    mut snapped: Local<bool>,
) {
    let Some(player) = players
        .iter()
        .find(|(_, faction)| **faction == Faction::Player)
        .map(|(transform, _)| transform.translation)
    else {
        return;
    };
    let dt = time.delta_secs();

    for (mut rig, mut transform) in &mut rigs {
        let target = rig.follow_target(player);
        let t = rig.follow_blend(*snapped, dt);
        rig.focus = rig.focus.lerp(target, t.clamp(0.0, 1.0));
        rig.clamp_focus();
        *transform = rig.transform();
    }
    *snapped = true;
}

/// 方向光（与体素明暗的假想光照方向一致，读起来更立体）。
pub fn light() -> impl Scene {
    let transform = Transform::from_xyz(0.0, 18.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y);
    bsn! {
        DirectionalLight {
            color: Color::WHITE,
            illuminance: 10_000.0,
            shadow_maps_enabled: true,
        }
        template_value(transform)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    /// 相机 App：平移 + 缩放 + 跟随三个系统按 100ms/帧推进，场上有玩家（6, 0, 2）。
    fn camera_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .add_message::<PanCamera>()
            .add_message::<ZoomCamera>()
            .add_systems(
                Update,
                (camera_pan_system, camera_zoom_system, camera_follow_system).chain(),
            );
        let rig = CameraRig::new(Vec3::ZERO);
        let transform = rig.transform();
        app.world_mut().spawn((rig, transform));
        app.world_mut()
            .spawn((Faction::Player, Transform::from_xyz(6.0, 0.0, 2.0)));
        app
    }

    fn rig_of(app: &mut App) -> CameraRig {
        let mut query = app.world_mut().query::<&CameraRig>();
        *query.iter(app.world()).next().expect("应当有相机机位")
    }

    fn camera_transform(app: &mut App) -> Transform {
        let mut query = app
            .world_mut()
            .query_filtered::<&Transform, With<CameraRig>>();
        *query
            .iter(app.world())
            .next()
            .expect("应当有相机 Transform")
    }

    #[test]
    fn the_camera_snaps_onto_the_player_on_the_first_frame() {
        let mut app = camera_app();
        app.update();

        let rig = rig_of(&mut app);
        assert_eq!(
            (rig.focus.x, rig.focus.z),
            (6.0, 2.0),
            "第一帧直接吸附到玩家脚下，而不是从初始机位慢慢飘过去"
        );
        assert_eq!(rig.focus.y, 0.0, "注视点始终在地面平面");
    }

    /// 滚轮：正向拉远、负向拉近，并且都停在上下限上。
    #[test]
    fn the_wheel_zooms_the_camera_and_clamps() {
        let mut app = camera_app();
        app.update();
        let base = rig_of(&mut app).offset.length();

        app.world_mut().write_message(ZoomCamera { delta: 100.0 });
        app.update();
        let rig = rig_of(&mut app);
        assert_eq!(rig.zoom, CameraRig::MAX_ZOOM, "一直拉远应当停在最远机位");
        assert!(
            camera_transform(&mut app).translation.distance(rig.focus) > base,
            "拉远之后相机离注视点更远"
        );

        app.world_mut().write_message(ZoomCamera { delta: -100.0 });
        app.update();
        let rig = rig_of(&mut app);
        assert_eq!(rig.zoom, CameraRig::MIN_ZOOM, "一直拉近应当停在最近机位");
        assert!(
            camera_transform(&mut app).translation.distance(rig.focus) < base,
            "拉近之后相机离注视点更近"
        );
    }

    /// 缩放之后 `Transform` 必须跟着机位重算（只改 `zoom` 而不刷新 `Transform`
    /// 会让屏幕上看不出任何变化）。
    #[test]
    fn zooming_rewrites_the_camera_transform() {
        let mut app = camera_app();
        app.update();

        app.world_mut().write_message(ZoomCamera { delta: 1.0 });
        app.update();

        let rig = rig_of(&mut app);
        assert_eq!(
            camera_transform(&mut app).translation,
            rig.focus + rig.offset * rig.zoom,
            "缩放后的相机位置应当等于「注视点 + 偏移 × 倍率」"
        );
    }

    #[test]
    fn dragging_offsets_the_follow_along_the_ground() {
        let mut app = camera_app();
        app.update();
        app.world_mut().write_message(PanCamera {
            delta: Vec2::new(100.0, 0.0),
        });
        // 跟随是指数趋近：跑够时间再看最终机位
        for _ in 0..30 {
            app.update();
        }

        let rig = rig_of(&mut app);
        assert!(
            rig.pan_offset.x < 0.0 && rig.pan_offset.y > 0.0,
            "拖右应当让地面向右走（观察点左移），实际 {:?}",
            rig.pan_offset
        );
        let expected = (6.0 + rig.pan_offset.x, 2.0 + rig.pan_offset.y);
        assert!(
            (rig.focus.x - expected.0).abs() < 1e-3 && (rig.focus.z - expected.1).abs() < 1e-3,
            "拖完之后机位收敛到「玩家 + 观察偏移」（偏移不会自己弹回）：{:?} vs {expected:?}",
            (rig.focus.x, rig.focus.z)
        );
    }

    #[test]
    fn dragging_down_moves_the_view_away_from_the_camera() {
        let mut app = camera_app();
        app.world_mut().write_message(PanCamera {
            delta: Vec2::new(0.0, 100.0),
        });
        app.update();

        let rig = rig_of(&mut app);
        // 相机在 focus 的 +x/+z 侧，往前（远离相机）就是 -x/-z
        assert!(
            rig.pan_offset.x < 0.0 && rig.pan_offset.y < 0.0,
            "拖下应当把地面拖向远处，实际 {:?}",
            rig.pan_offset
        );
    }

    #[test]
    fn pan_offset_stays_within_the_follow_radius() {
        let mut app = camera_app();
        for _ in 0..200 {
            app.world_mut().write_message(PanCamera {
                delta: Vec2::new(-400.0, -400.0),
            });
            app.update();
        }

        let rig = rig_of(&mut app);
        assert!(
            (rig.pan_offset.length() - CameraRig::PAN_RADIUS).abs() < 1e-3,
            "长时间拖动应当停在观察半径上，而不是无限漂移：{:?}",
            rig.pan_offset
        );
        assert!(
            (rig.focus.x - (6.0 + rig.pan_offset.x)).abs() < 1e-3
                && (rig.focus.z - (2.0 + rig.pan_offset.y)).abs() < 1e-3,
            "偏移再大，机位也只是「玩家 + 偏移」：{:?}",
            (rig.focus.x, rig.focus.z)
        );
    }
}
