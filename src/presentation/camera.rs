//! 相机与方向光：场景零件 + 中键拖拽平移。
//!
//! 平移是纯表现：输入域只把鼠标位移翻译成 [`PanCamera`] 消息，本模块消费它并改
//! 相机机位——游戏状态（单位、时间线）一点没碰。

use bevy::prelude::*;

use super::components::{CameraRig, MainCamera};

/// 相机平移请求（写：[`crate::input`]；消费：[`camera_pan_system`]）。
///
/// `delta` 是鼠标位移（像素），语义是「抓住地面拖动」。
#[derive(Message, Debug, Clone, Copy)]
pub struct PanCamera {
    pub delta: Vec2,
}

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

/// 中键拖拽 → 平移机位：往哪边拖，地面就往哪边走（grab-drag 手感）。
///
/// 位移按相机自身的朝向换算到地面：拖右 = 镜头左移，拖下 = 镜头前移；
/// 注视点被限制在 [`CameraRig::bounds`] 内。
pub fn camera_pan_system(
    mut requests: MessageReader<PanCamera>,
    mut rigs: Query<(&mut CameraRig, &mut Transform)>,
) {
    let mut delta = Vec2::ZERO;
    for request in requests.read() {
        delta += request.delta;
    }
    if delta == Vec2::ZERO {
        return;
    }

    for (mut rig, mut transform) in &mut rigs {
        // 相机的屏幕轴投影到地面：拖拽方向直接对应「地面跟着手走」
        let right = (transform.rotation * Vec3::X)
            .with_y(0.0)
            .normalize_or_zero();
        let forward = (transform.rotation * Vec3::NEG_Z)
            .with_y(0.0)
            .normalize_or_zero();
        rig.focus += (right * -delta.x + forward * delta.y) * PAN_PER_PIXEL;
        rig.clamp_focus();
        *transform = rig.transform();
    }
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

    fn pan_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<PanCamera>()
            .add_systems(Update, camera_pan_system);
        let rig = CameraRig::new(Vec3::ZERO);
        let transform = rig.transform();
        app.world_mut().spawn((rig, transform));
        app
    }

    fn rig_of(app: &mut App) -> CameraRig {
        let mut query = app.world_mut().query::<&CameraRig>();
        *query.iter(app.world()).next().expect("应当有相机机位")
    }

    #[test]
    fn dragging_pans_the_focus_along_the_ground() {
        let mut app = pan_app();
        app.world_mut().write_message(PanCamera {
            delta: Vec2::new(100.0, 0.0),
        });
        app.update();

        let rig = rig_of(&mut app);
        assert_eq!(rig.focus.y, 0.0, "平移只在 XZ 平面，不动高度");
        assert!(
            rig.focus.x < 0.0 && rig.focus.z > 0.0,
            "拖右应当让地面向右走（镜头左移），实际 {:?}",
            rig.focus
        );
    }

    #[test]
    fn dragging_down_moves_the_focus_away_from_the_camera() {
        let mut app = pan_app();
        app.world_mut().write_message(PanCamera {
            delta: Vec2::new(0.0, 100.0),
        });
        app.update();

        let rig = rig_of(&mut app);
        // 相机在 focus 的 +x/+z 侧，往前（远离相机）就是 -x/-z
        assert!(
            rig.focus.x < 0.0 && rig.focus.z < 0.0,
            "拖下应当把地面拖向远处，实际 {:?}",
            rig.focus
        );
    }

    #[test]
    fn panning_stays_inside_the_rig_bounds() {
        let mut app = pan_app();
        for _ in 0..200 {
            app.world_mut().write_message(PanCamera {
                delta: Vec2::new(-400.0, -400.0),
            });
            app.update();
        }

        let rig = rig_of(&mut app);
        assert!(rig.focus.x >= rig.bounds.min.x && rig.focus.x <= rig.bounds.max.x);
        assert!(rig.focus.z >= rig.bounds.min.y && rig.focus.z <= rig.bounds.max.y);
        let clamped = rig.focus.x == rig.bounds.min.x
            || rig.focus.x == rig.bounds.max.x
            || rig.focus.z == rig.bounds.min.y
            || rig.focus.z == rig.bounds.max.y;
        assert!(clamped, "长时间拖动应当停在边界上，而不是无限漂移");
    }

    #[test]
    fn transform_follows_the_focus() {
        let mut app = pan_app();
        app.world_mut().write_message(PanCamera {
            delta: Vec2::new(120.0, 60.0),
        });
        app.update();

        let rig = rig_of(&mut app);
        let mut query = app.world_mut().query::<&Transform>();
        let transform = *query
            .iter(app.world())
            .next()
            .expect("应当有相机 Transform");
        assert_eq!(
            transform.translation,
            rig.focus + rig.offset,
            "相机位置 = 注视点 + 固定偏移"
        );
    }
}
