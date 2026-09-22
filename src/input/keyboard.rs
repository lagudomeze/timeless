//! 键盘 → 消息：全部玩家输入的翻译层。

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::combat::defense::ParryCommand;
use crate::combat::skills::{CycleSkill, SelectSkill, SkillKind, UseSelectedSkill};
use crate::movement::{JumpCommand, MoveCommand};
use crate::presentation::{CameraRig, ToggleHelp};
use crate::spawn::ResetBattle;
use crate::timeline::{PauseRequest, PlayerTakeover, UseFocus};

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
    mut moves: MessageWriter<MoveCommand>,
    mut takeovers: MessageWriter<PlayerTakeover>,
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
    moves.write(MoveCommand {
        axis: basis.axis(screen),
    });
    takeovers.write(PlayerTakeover);
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
    mut takeovers: MessageWriter<PlayerTakeover>,
) {
    if keys.just_pressed(KeyCode::KeyC) {
        jumps.write(JumpCommand);
        takeovers.write(PlayerTakeover);
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
                    takeovers.write(PlayerTakeover);
                }
            }
            HotkeyAction::Parry => {
                parry_commands.write(ParryCommand);
                takeovers.write(PlayerTakeover);
            }
        }
    }
}

/// 空格 → 翻转世界的冻结状态（**只写一条消息，不碰时钟，也不看别人的原因**）。
///
/// 「按一下是暂停还是继续」这个判定归输入域：冻着就放开，没冻就停住——翻转本身
/// 由时间线在 [`process_pause_requests`](crate::timeline::process_pause_requests) 里落地。
///
/// 这里刻意**不读 [`PauseReasons`](crate::timeline::PauseReasons)**：集合里同时躺着
/// 别人的原因（`awaiting` / `threat`），从它反推"手动暂停开着吗"会把
/// 「威胁正冻着」误判成「玩家已手动暂停」，于是空格变成又一次"暂停"而不是"继续"。
/// 输入域只表达"玩家翻了一下"，剩下的（包括"恢复之后哪些原因马上会回来"）是时间线的事。
pub fn pause_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut requests: MessageWriter<PauseRequest>,
) {
    if keys.just_pressed(KeyCode::Space) {
        requests.write(PauseRequest::Toggle);
    }
}

/// `F5` → [`ResetBattle`]（只翻译，不改状态）。
///
/// 组装车间只负责「清场 + 用同一套工厂重新组装」，**不认识按键**：
/// 触发键住在输入域，和所有其它按键一样只把意图翻成一条消息。
pub fn restart_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut resets: MessageWriter<ResetBattle>,
) {
    if keys.just_pressed(KeyCode::F5) {
        resets.write(ResetBattle);
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

/// `Shift` + 任一「决策键」→ [`UseFocus`]：这一手用 1 点 Focus 换前摇归零。
///
/// 输入域只翻译意图（哪个键是按下的、Shift 有没有按着），**扣不扣 Focus 由声明那一刻
/// 决定**：没有真的声明行动就不会花掉——空按 Shift 什么也不消耗。
/// 所以这里只需要认出"这一帧有没有决策键被按下"，不必知道各领域会怎么处理它。
pub fn focus_intent_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<HotkeyBinds>,
    mut requests: MessageWriter<UseFocus>,
) {
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if !shift {
        return;
    }
    // 方向键 / 跳跃 / 释放技能 / 数字键 / 技能热键——凡是能声明行动的都算
    let declared = [
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
        KeyCode::ArrowLeft,
        KeyCode::ArrowRight,
        KeyCode::KeyC,
        KeyCode::KeyG,
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ]
    .into_iter()
    .chain(binds.entries.iter().map(|(key, _)| *key))
    .any(|key| keys.just_pressed(key));
    if declared {
        requests.write(UseFocus);
    }
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
    mut takeovers: MessageWriter<PlayerTakeover>,
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
            takeovers.write(PlayerTakeover);
        }
    }

    // 循环：Shift + Tab = 反向。**只选不执行**，因此不算「玩家动手了」
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
    mut takeovers: MessageWriter<PlayerTakeover>,
) {
    if keys.just_pressed(KeyCode::KeyG) {
        uses.write(UseSelectedSkill::default());
        takeovers.write(PlayerTakeover);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::PauseReasons;

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

    /// 空格：关着按一下 = 断言手动暂停；开着再按 = 解冻。
    ///
    /// 「按一下是暂停还是恢复」的闩就藏在 [`PauseReasons`] 里，因此测试必须让它
    /// 真的转一圈——这里把调度域真正的 `process_pause_requests` 接上，
    /// 而不是自己糊一个假的集合。
    #[test]
    fn space_toggles_the_manual_pause_assertion() {
        #[derive(Resource, Default)]
        struct Captured(Vec<PauseRequest>);

        fn capture(mut requests: MessageReader<PauseRequest>, mut captured: ResMut<Captured>) {
            captured.0.extend(requests.read().copied());
        }

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<PauseReasons>()
            .init_resource::<crate::timeline::ManualPause>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Captured>()
            .add_message::<PauseRequest>()
            .add_systems(
                Update,
                (
                    pause_input_system,
                    capture,
                    crate::timeline::process_pause_requests,
                )
                    .chain(),
            );

        /// 松手：把空格从"按下"集合里摘掉。
        ///
        /// 必须用 `reset` 而不是 `clear`——后者只清 `just_pressed` / `just_released`，
        /// `pressed` 还留着，下一次 `press` 就不会再置 `just_pressed` 了。
        fn release_space(app: &mut App) {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset(KeyCode::Space);
        }

        // 没有输入：什么也不写
        app.update();
        assert!(app.world().resource::<Captured>().0.is_empty());
        assert!(!app.world().resource::<PauseReasons>().is_frozen());

        // ① 关着按空格 → 翻开关：此后**每帧继续断言**（不是只生效一帧）
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.update();
        release_space(&mut app);
        assert_eq!(
            app.world().resource::<Captured>().0,
            vec![PauseRequest::Toggle],
            "空格只发一条翻转，不是每帧断言"
        );
        // 手动暂停**不写原因**：世界停不停由 `Time<Virtual>` 回答（见 clock.rs）
        for _ in 0..3 {
            app.update();
            assert!(
                app.world().resource::<Time<Virtual>>().is_paused(),
                "松手之后世界要一直冻着，手动暂停才有效"
            );
        }

        // ② 开着再按 → 翻回去：世界恢复流动
        app.world_mut().resource_mut::<Captured>().0.clear();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.update();
        assert_eq!(
            app.world().resource::<Captured>().0,
            vec![PauseRequest::Toggle],
            "再按一下还是翻转（不是 Resume）"
        );
        release_space(&mut app);
        app.update();
        assert!(
            !app.world().resource::<Time<Virtual>>().is_paused(),
            "翻回去之后世界该恢复流动"
        );
    }

    /// 空格的语义：**翻转世界的冻结状态**，而且手动暂停**不留原因**。
    ///
    /// 这条守着新设计的两半：停住时靠 `Time<Virtual>` 记状态（不写集合），
    /// 放开时清空集合。判据来自"入帧时冻着吗"，不是从原因集合反推。
    #[test]
    fn space_toggles_the_world_frozen_state() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<PauseReasons>()
            .init_resource::<crate::timeline::ManualPause>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<PauseRequest>()
            .add_systems(
                Update,
                (pause_input_system, crate::timeline::process_pause_requests).chain(),
            );

        // 世界没冻 → 按空格把它停住（不写原因，时钟自己记着）
        app.update();
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.update();
        assert!(
            app.world().resource::<Time<Virtual>>().is_paused(),
            "没冻的时候按空格 → 停住"
        );
        assert!(
            app.world().resource::<PauseReasons>().labels().is_empty(),
            "手动暂停不往集合里写原因（集合是给「谁在停表」看的）"
        );

        // 松手再按一次：冻着 → 放开
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset(KeyCode::Space);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.update();
        assert!(
            !app.world().resource::<Time<Virtual>>().is_paused(),
            "冻着的时候按空格 → 放开"
        );
    }
}
