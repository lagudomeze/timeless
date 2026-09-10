//! 键盘 → 消息：全部玩家输入的翻译层。

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::combat::skills::{FireCommand, MeleeCommand};
use crate::movement::{JumpCommand, MoveCommand};
use crate::presentation::CameraRig;
use crate::timeline::ActionsCommitted;

/// WASD / 方向键 → 世界平面移动方向。
///
/// 玩家的按键是**屏幕方向**（W 向上 = 远离相机、D 向右 = 相机的右手边），
/// 所以这里按相机朝向换算到世界 XZ 平面（`MoveCommand.axis` 的约定见
/// [`crate::movement::ground_direction`]）。没有相机时（单测）退回世界轴：
/// W → 世界 +Z、D → 世界 +X。
///
/// 每帧都写一条消息（含零方向），消费端据此设置 / 归零速度，
/// 因此不需要单独处理「松开按键」事件。
pub fn player_move_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    cameras: Query<&Transform, With<CameraRig>>,
    mut commands: MessageWriter<MoveCommand>,
) {
    let mut screen = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        screen.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        screen.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        screen.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        screen.x += 1.0;
    }
    let screen = screen.normalize_or_zero();

    let basis = cameras
        .iter()
        .next()
        .map(|transform| GroundBasis::from_rotation(transform.rotation))
        .unwrap_or_else(GroundBasis::world);
    commands.write(MoveCommand {
        axis: basis.axis(screen),
    });
}

/// 相机的「地面基」：屏幕向右 / 屏幕向上分别对应哪个世界方向（都投影到地面）。
#[derive(Debug, Clone, Copy)]
struct GroundBasis {
    right: Vec3,
    forward: Vec3,
}

impl GroundBasis {
    /// 没有相机时的世界轴：右 = +X，上 = +Z。
    fn world() -> Self {
        Self {
            right: Vec3::X,
            forward: Vec3::Z,
        }
    }

    /// 从相机旋转取出地面基；相机垂直向下看时退回世界轴。
    fn from_rotation(rotation: Quat) -> Self {
        let right = (rotation * Vec3::X).with_y(0.0).try_normalize();
        let forward = (rotation * Vec3::NEG_Z).with_y(0.0).try_normalize();
        match (right, forward) {
            (Some(right), Some(forward)) => Self { right, forward },
            _ => Self::world(),
        }
    }

    /// 屏幕方向（x 右、y 上）→ 世界平面轴（x → 世界 X、y → 世界 Z）。
    fn axis(self, screen: Vec2) -> Vec2 {
        let direction = self.right * screen.x + self.forward * screen.y;
        Vec2::new(direction.x, direction.z).normalize_or_zero()
    }
}

/// Q → 发射箭矢；E → 近战横扫；空格 → 跳跃。
pub fn player_skill_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut fire_commands: MessageWriter<FireCommand>,
    mut melee_commands: MessageWriter<MeleeCommand>,
    mut jump_commands: MessageWriter<JumpCommand>,
) {
    if keys.just_pressed(KeyCode::KeyQ) {
        fire_commands.write(FireCommand);
    }
    if keys.just_pressed(KeyCode::KeyE) {
        melee_commands.write(MeleeCommand);
    }
    if keys.just_pressed(KeyCode::Space) {
        jump_commands.write(JumpCommand);
    }
}

/// Enter → 提交本轮（`ActionsCommitted`）。
///
/// 提交只是「我准备好了」：时间线会把所有单位本轮的声明一起变成 `Pending`，
/// 再推进一个窗口。规划阶段之外按下不做任何事。
pub fn player_commit_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut commits: MessageWriter<ActionsCommitted>,
) {
    if keys.just_pressed(KeyCode::Enter) {
        commits.write(ActionsCommitted);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_basis_uses_x_and_z() {
        let basis = GroundBasis::world();
        assert_eq!(basis.axis(Vec2::Y), Vec2::Y, "W 对应世界 +Z");
        assert_eq!(basis.axis(Vec2::X), Vec2::X, "D 对应世界 +X");
    }

    #[test]
    fn camera_basis_makes_w_go_away_from_the_camera() {
        // 真实机位：相机在 focus 的 (+12, +14, +12) 侧，俯视 45°
        let rig = CameraRig::new(Vec3::ZERO);
        let transform = rig.transform();
        let basis = GroundBasis::from_rotation(transform.rotation);

        let forward = (transform.rotation * Vec3::NEG_Z).with_y(0.0).normalize();
        let up_world_axis = basis.axis(Vec2::Y);
        assert!(
            up_world_axis.x * forward.x + up_world_axis.y * forward.z > 0.9,
            "W 应当朝远离相机的方向；实际 {up_world_axis:?}"
        );

        let right = (transform.rotation * Vec3::X).with_y(0.0).normalize();
        let right_axis = basis.axis(Vec2::X);
        assert!(
            right_axis.x * right.x + right_axis.y * right.z > 0.9,
            "D 应当朝相机的右手边；实际 {right_axis:?}"
        );
    }

    #[test]
    fn vertical_camera_falls_back_to_world_axes() {
        let basis = GroundBasis::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
        assert_eq!(basis.axis(Vec2::Y), Vec2::Y, "垂直俯视时退回世界轴");
    }
}
