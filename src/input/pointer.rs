//! 鼠标输入：只翻译成消息。

use bevy::input::mouse::{AccumulatedMouseMotion, MouseButton, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::ui::Interaction;

use crate::combat::attack::SelectSkill;
use crate::interaction::PointerCommand;
use crate::presentation::hud::skills::SkillSlot;
use crate::presentation::{PanCamera, ZoomCamera};

/// 滚轮 → [`ZoomCamera`]（拉近 / 拉远；行 / 像素两种滚动单位都归一成"格"）。
pub fn camera_zoom_input_system(
    mut wheels: MessageReader<MouseWheel>,
    mut zooms: MessageWriter<ZoomCamera>,
) {
    // 累加本帧所有滚动事件，一次发一条请求
    let mut total = 0.0;
    for event in wheels.read() {
        let delta = match event.unit {
            MouseScrollUnit::Line => event.y,
            // 触摸板 / 高精度滚轮给的是像素：50px 当一格
            MouseScrollUnit::Pixel => event.y / 50.0,
        };
        total += delta;
    }
    if total != 0.0 {
        zooms.write(ZoomCamera { delta: total });
    }
}

/// 左 / 右键 → [`PointerCommand`]（只翻译，怎么解释由交互域决定）。
pub fn pointer_click_input_system(
    buttons: Res<ButtonInput<MouseButton>>,
    mut clicks: MessageWriter<PointerCommand>,
) {
    if buttons.just_pressed(MouseButton::Left) {
        clicks.write(PointerCommand::Primary);
    }
    if buttons.just_pressed(MouseButton::Right) {
        clicks.write(PointerCommand::Secondary);
    }
}

/// 点技能槽 = **选中**它（写：本系统；消费：`combat::attack::select_skill_system`）。
///
/// **不释放技能**：选中是"我想用这一手"，释放是"我现在就用"——后者由
/// `G` / 数字键 / 点战场负责。误点一下就把决策花掉是最糟的手感。
///
/// 为什么住在 `input` 而不是 `interaction`：本项目的依赖方向是
/// `input ──▶ … / presentation / …`，而 `interaction` 的域描述里**不含
/// `presentation`**；放这里与"`1`~`5` 选第 N 个槽"（`skill_menu_input_system`）同处一地，
/// 语义一致（见 `docs/backlog/hud.md` 的 #55 拍板）。
///
/// `Changed<Interaction>` 保证一次点击只选一次；`Interaction` 由 `bevy_ui` 的
/// `ui_focus_system` 在 `PreUpdate` 写好，`Update` 里读到的是同一帧的值。
pub fn skill_slot_click_input_system(
    slots: Query<(&SkillSlot, &Interaction), Changed<Interaction>>,
    mut selects: MessageWriter<SelectSkill>,
) {
    for (slot, interaction) in &slots {
        if *interaction == Interaction::Pressed {
            selects.write(SelectSkill(slot.index));
        }
    }
}

/// 按住鼠标中键拖动 → [`PanCamera`]（把本帧累计的鼠标位移交给表现域）。
pub fn camera_pan_input_system(
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut pans: MessageWriter<PanCamera>,
) {
    if !buttons.pressed(MouseButton::Middle) || motion.delta == Vec2::ZERO {
        return;
    }
    pans.write(PanCamera {
        delta: motion.delta,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 点技能槽 → 只写 [`SelectSkill`]，**不释放技能**（#55）。
    ///
    /// 这条钉的是"外观是按钮、行为也是按钮"：此前技能槽挂着 `Button`、
    /// 悬停有描边，但**没有任何系统处理点击**——点下去唯一的后果是曾经的 UI 穿透。
    ///
    /// 「不释放技能」那一半同样重要：误点一下就把决策花掉是最糟的手感，
    /// 所以这里同时断言没有 `UseSelectedSkill` / `PlayerTakeover`。
    #[test]
    fn clicking_a_skill_slot_selects_it_without_casting() {
        #[derive(Resource, Default)]
        struct Seen {
            selected: Vec<usize>,
            used: usize,
            takeovers: usize,
        }
        fn capture(
            mut selects: MessageReader<SelectSkill>,
            mut uses: MessageReader<crate::combat::attack::UseSelectedSkill>,
            mut takeovers: MessageReader<crate::timeline::PlayerTakeover>,
            mut seen: ResMut<Seen>,
        ) {
            seen.selected.extend(selects.read().map(|select| select.0));
            seen.used += uses.read().count();
            seen.takeovers += takeovers.read().count();
        }

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Seen>()
            .add_message::<SelectSkill>()
            .add_message::<crate::combat::attack::UseSelectedSkill>()
            .add_message::<crate::timeline::PlayerTakeover>()
            .add_systems(Update, (skill_slot_click_input_system, capture).chain());

        let third = app
            .world_mut()
            .spawn((SkillSlot { index: 2 }, Interaction::Pressed))
            .id();
        app.update();

        let seen = app.world().resource::<Seen>();
        assert_eq!(seen.selected, vec![2], "点第 3 格 → 选中第 3 格");
        assert_eq!(seen.used, 0, "选中不等于释放：不该写 UseSelectedSkill");
        assert_eq!(seen.takeovers, 0, "选中不该撤掉玩家当前那一手");

        // 悬停（没按下）不算点击；`Changed<Interaction>` 也保证不重复触发
        app.world_mut()
            .get_mut::<Interaction>(third)
            .unwrap()
            .clone_from(&Interaction::Hovered);
        app.world_mut().resource_mut::<Seen>().selected.clear();
        app.update();
        assert!(
            app.world().resource::<Seen>().selected.is_empty(),
            "只是悬停不该选中"
        );
    }
}
