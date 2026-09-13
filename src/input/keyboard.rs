//! 键盘 → 消息：全部玩家输入的翻译层。

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::combat::defense::ParryCommand;
use crate::combat::skills::{CycleSkill, SelectSkill, SkillKind, UseSelectedSkill};
use crate::movement::{JumpCommand, MoveCommand};
use crate::presentation::{CameraRig, ToggleHelp};
use crate::timeline::{CycleReactionWindow, TogglePause};

/// `Q/W/E/R` 的技能热键绑定（默认值；用户自定义留到配置外置那一步）。
///
/// 无回合模型里"按一下就出手"是最舒服的输入，所以热键**直接执行**：
/// 等价于「选中这个技能 + 用一次」（和数字键、左键点击同一条路径）。
#[derive(Resource, Debug, Clone)]
pub struct HotkeyBinds {
    pub entries: Vec<(KeyCode, HotkeyAction)>,
}

/// 热键能绑定的动作：技能栏里的四种 + 不占栏位的反应动作（招架）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    /// 技能栏里的技能（按注册表下标执行）
    Skill(SkillKind),
    /// 招架（不在技能栏里：它是绑定某次攻击的反应）
    Parry,
}

impl Default for HotkeyBinds {
    fn default() -> Self {
        Self {
            entries: vec![
                (KeyCode::KeyQ, HotkeyAction::Skill(SkillKind::Fireball)),
                (KeyCode::KeyW, HotkeyAction::Skill(SkillKind::Melee)),
                (KeyCode::KeyE, HotkeyAction::Skill(SkillKind::Roll)),
                (KeyCode::KeyR, HotkeyAction::Parry),
            ],
        }
    }
}

/// 方向键 → 世界平面移动方向（**按下的那一次**）。
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
    if keys.pressed(KeyCode::ArrowUp) {
        screen.y += 1.0;
    }
    if keys.pressed(KeyCode::ArrowDown) {
        screen.y -= 1.0;
    }
    if keys.pressed(KeyCode::ArrowLeft) {
        screen.x -= 1.0;
    }
    if keys.pressed(KeyCode::ArrowRight) {
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

/// `Q/W/E/R` 技能热键（默认绑定见 [`HotkeyBinds`]）+ `C` 跳跃。
///
/// 热键**直接执行**：选中 + 用一次（和数字键、左键同一条路径）；
/// 空格已经让给"暂停"，所以跳跃挪到 `C`（后续空战也挂这里）。
pub fn player_skill_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<HotkeyBinds>,
    mut selects: MessageWriter<SelectSkill>,
    mut uses: MessageWriter<UseSelectedSkill>,
    mut jumps: MessageWriter<JumpCommand>,
    mut parry_commands: MessageWriter<ParryCommand>,
) {
    if keys.just_pressed(KeyCode::KeyC) {
        jumps.write(JumpCommand);
    }
    for (key, action) in &binds.entries {
        if !keys.just_pressed(*key) {
            continue;
        }
        match action {
            HotkeyAction::Skill(kind) => {
                if let Some(index) = crate::combat::skills::index_of(*kind) {
                    selects.write(SelectSkill(index));
                    uses.write(UseSelectedSkill::default());
                }
            }
            HotkeyAction::Parry => {
                parry_commands.write(ParryCommand);
            }
        }
    }
}

/// 空格 → 暂停 / 继续（**空格只表示暂停**，不触发任何行动）。
pub fn pause_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut toggles: MessageWriter<TogglePause>,
) {
    if keys.just_pressed(KeyCode::Space) {
        toggles.write(TogglePause);
    }
}

/// F1 → 开合帮助面板。
///
/// 只翻译成 [`ToggleHelp`] 消息，由 HUD 消费；输入域不认识面板长什么样。
pub fn player_help_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut toggles: MessageWriter<ToggleHelp>,
) {
    if keys.just_pressed(KeyCode::F1) {
        toggles.write(ToggleHelp);
    }
}

/// F2 → 循环「反应窗口」的松紧（loose → strict → off）。
///
/// 输入域只改**配置**，是否真的停表由时间线的门控解释。
pub fn reaction_window_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut requests: MessageWriter<CycleReactionWindow>,
) {
    if !keys.just_pressed(KeyCode::F2) {
        return;
    }
    requests.write(CycleReactionWindow);
}

/// 技能栏：`1`~`4` **直接执行**那一格 · `Tab`/`Shift+Tab` 循环（只选，不执行）。
///
/// 与其它输入一样**只翻译**：选择与释放都是消息，落地由技能域负责。
/// 「直接执行」= 选中 + 用一次（同一条路径），所以键盘和鼠标点击行为一致。
pub fn skill_menu_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut selects: MessageWriter<SelectSkill>,
    mut cycles: MessageWriter<CycleSkill>,
    mut uses: MessageWriter<UseSelectedSkill>,
) {
    // 直接执行那一格（选中 + 用一次）
    let direct = [
        (KeyCode::Digit1, 0usize),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
        (KeyCode::Digit4, 3),
    ];
    for (key, index) in direct {
        if keys.just_pressed(key) {
            selects.write(SelectSkill(index));
            uses.write(UseSelectedSkill::default());
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
        uses.write(UseSelectedSkill::default());
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
