//! 输入域插件。

use bevy::prelude::*;

use super::InputSet;
use super::keyboard::{
    HotkeyBinds, focus_intent_input_system, pause_input_system, player_help_input_system,
    player_move_input_system, player_skill_input_system, restart_input_system,
    skill_menu_input_system, skill_use_input_system,
};
use super::pointer::{
    camera_pan_input_system, camera_zoom_input_system, pointer_click_input_system,
};

/// 玩家输入插件：只注册「按键 → 消息」的翻译系统。
///
/// 消息本身由**消费它们的领域**注册：`MoveCommand` → movement、
/// `FireCommand` / `MeleeCommand` / 技能菜单消息 → combat、
/// `PauseRequest` / `PlayerTakeover` / `UseFocus` → timeline、
/// `PointerCommand` → interaction、`PanCamera` / `ZoomCamera` → presentation、
/// `ResetBattle` → spawn。
/// 生产者只引用消息类型，不引用消费系统；因此单独装本插件会缺消息
/// （系统初始化即报错），要连同上面几个领域一起装。
///
/// 暂停是唯一读回游戏状态的输入：`pause_input_system` 要读
/// [`PauseReasons`](crate::clock::PauseReasons) 才知道「按一下是暂停还是恢复」——
/// 这个判定本来就属于输入域，调度域只管收断言。
#[derive(Debug, Default)]
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HotkeyBinds>().add_systems(
            Update,
            (
                player_move_input_system,
                player_skill_input_system,
                skill_menu_input_system,
                skill_use_input_system,
                // F1 只写「开合帮助面板」的消息，由 HUD 消费
                player_help_input_system,
                // 空格 = 暂停 / 继续；F5 = 重置战斗（都只写消息）
                pause_input_system,
                restart_input_system,
                // Shift + 决策键 = 用 Focus 换前摇（只写请求，扣费在声明那一刻）
                focus_intent_input_system,
                camera_pan_input_system,
                // 滚轮 → `ZoomCamera`（拉近 / 拉远）
                camera_zoom_input_system,
                // 左/右键 → `PointerCommand`（由 interaction 解释成走/打/撤销）
                pointer_click_input_system,
            )
                .in_set(InputSet),
        );
    }
}
