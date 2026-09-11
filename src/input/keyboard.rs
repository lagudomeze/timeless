//! 键盘 → 消息：全部玩家输入的翻译层。

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::combat::defense::{ParryCommand, RollCommand};
use crate::combat::skills::{CycleSkill, FireCommand, MeleeCommand, SelectSkill, UseSelectedSkill};
use crate::movement::{JumpCommand, MoveCommand};
use crate::presentation::CameraRig;
use crate::timeline::{ActionsCommitted, TimelineConfig};

/// WASD / 方向键 → 世界平面移动方向（**按下的那一次**）。
///
/// 玩家的按键是**屏幕方向**（W 向上 = 远离相机、D 向右 = 相机的右手边），
/// 所以这里按相机朝向换算到世界 XZ 平面（`MoveCommand.axis` 的约定见
/// [`crate::movement::ground_direction`]）。没有相机时（单测）退回世界轴：
/// W → 世界 +Z、D → 世界 +X。
///
/// 只在**方向发生变化**时发消息（`Local` 记住上一次的方向）：
/// 无回合模型里「按一次 = 走一格」，若每帧都发，「按住 W」会不停顶掉
/// 玩家刚声明的技能；同时按下两个方向键时以 Shift 一侧为准。
pub fn player_move_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    cameras: Query<&Transform, With<CameraRig>>,
    mut commands: MessageWriter<MoveCommand>,
    mut last_axis: Local<Vec2>,
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
    if screen == *last_axis {
        return; // 方向没变：不再重复声明
    }
    *last_axis = screen;
    if screen == Vec2::ZERO {
        return; // 松手不发消息：一次决策已经消耗掉了
    }

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

/// Q → 发射箭矢；E → 近战横扫；空格 → 跳跃；F → 翻滚；V → 招架。
///
/// 翻滚 / 招架是**反应性操作**（消耗精力、随时可用），因此不参与
/// `require_commit` 的草案流程——它们照旧「按下即声明」。
pub fn player_skill_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut fire_commands: MessageWriter<FireCommand>,
    mut melee_commands: MessageWriter<MeleeCommand>,
    mut jump_commands: MessageWriter<JumpCommand>,
    mut roll_commands: MessageWriter<RollCommand>,
    mut parry_commands: MessageWriter<ParryCommand>,
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
    if keys.just_pressed(KeyCode::KeyF) {
        roll_commands.write(RollCommand);
    }
    if keys.just_pressed(KeyCode::KeyV) {
        parry_commands.write(ParryCommand);
    }
}

/// Enter → 提交草案（仅在 `TimelineConfig::require_commit` 开启时有意义）。
///
/// 默认「按下即决定」，所以这条消息平时不会改变任何东西；
/// 需要「先声明、再确认」的手感时用 `F1` 打开开关。
pub fn player_commit_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut commits: MessageWriter<ActionsCommitted>,
) {
    if keys.just_pressed(KeyCode::Enter) {
        commits.write(ActionsCommitted);
    }
}

/// F1 → 切换「是否需要 Enter 提交」。
///
/// 输入域只改**配置**（`TimelineConfig`）而不碰游戏状态；
/// 是否延迟执行由时间线的 `commit_bridge_system` 解释。
pub fn commit_mode_toggle_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<TimelineConfig>,
) {
    if !keys.just_pressed(KeyCode::F1) {
        return;
    }
    config.require_commit = !config.require_commit;
    info!(
        "commit mode: {}",
        if config.require_commit {
            "ON (declare with input, Enter to commit)"
        } else {
            "OFF (input applies immediately)"
        }
    );
}

/// 技能菜单：`1`~`4` 直选 · `Tab`/`Shift+Tab` 循环（跳过负担不起的）。
///
/// 与其它输入一样**只翻译**：选择消息由技能域的选择系统消费，
/// 输入层不判断消耗、不生成行动。释放是另一个系统（[`skill_use_input_system`]），
/// 因为「选择」随时可做，「释放」要求玩家当前就绪。
pub fn skill_menu_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut selects: MessageWriter<SelectSkill>,
    mut cycles: MessageWriter<CycleSkill>,
) {
    // 直选
    let direct = [
        (KeyCode::Digit1, 0usize),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
        (KeyCode::Digit4, 3),
    ];
    for (key, index) in direct {
        if keys.just_pressed(key) {
            selects.write(SelectSkill(index));
        }
    }

    // 循环：Shift + Tab = 反向
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if keys.just_pressed(KeyCode::Tab) {
        cycles.write(CycleSkill { forward: !shift });
    }
}

/// 释放当前选中的技能（`G`）。
///
/// 与 ↑ 分开一个系统：释放要求玩家**当前就绪**，而选择随时可以做
/// （忙的时候也想先把下一个技能选好）。
pub fn skill_use_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut uses: MessageWriter<UseSelectedSkill>,
) {
    if keys.just_pressed(KeyCode::KeyG) {
        uses.write(UseSelectedSkill);
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
        // 真实机位：相机在 focus 的 (+12, +14, +12) 侧并看向 focus。
        // 该机位下「相机前方」正好等于屏幕上方，因此 W 走相机前方（= 远离相机）。
        let rig = CameraRig::new(Vec3::ZERO);
        let transform = rig.transform();
        let basis = GroundBasis::from_rotation(transform.rotation);

        let forward = (transform.rotation * Vec3::NEG_Z).with_y(0.0).normalize();
        let up_world_axis = basis.axis(Vec2::Y);
        assert!(
            up_world_axis.x * forward.x + up_world_axis.y * forward.z > 0.9,
            "W 应当朝相机前方（远离相机）；实际 {up_world_axis:?}"
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
