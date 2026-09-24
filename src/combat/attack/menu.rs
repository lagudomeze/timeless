//! 技能菜单：**选择**与**释放**都只翻译成消息，不直接改状态。
//!
//! ```text
//! 键盘 / 面板 ──SelectSkill / CycleSkill──▶ 选择系统（只改 MenuSelection）
//!            ──UseSelectedSkill──────────▶ 派发系统（按当前选择写 MeleeCommand / FireCommand / …）
//! ```
//!
//! 与 AGENTS.md 的「UI 输入只翻译、不执行」一致：菜单不生成行动实体，
//! 也不扣精力——那些都由各领域的声明系统负责（唯一的精力扣费点也在那里）。

use bevy::prelude::*;

use crate::combat::Faction;
use crate::combat::attack::events::{FireCommand, MeleeCommand};
use crate::combat::defense::{RollCommand, Stamina};
use crate::movement::CELL_SIZE;
use crate::movement::Cell;
use crate::timeline::{DecisionSlot, FirstReady, InputDriven};

use super::registry::{SKILLS, SkillKind};

/// 贴脸判据（世界单位）：与 AI 的 `MELEE_REACH` 同一约定（3/4 格）。
pub const MELEE_REACH: f32 = CELL_SIZE * 0.75;

/// 当前选中的技能（资源）。
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MenuSelection {
    index: usize,
}

impl MenuSelection {
    /// 直接选中某个下标（越界则忽略）。
    pub fn select(&mut self, index: usize) {
        if index < SKILLS.len() {
            self.index = index;
        }
    }

    /// 当前下标。
    pub fn index(&self) -> usize {
        self.index
    }

    /// 当前选中的技能。
    pub fn selected(&self) -> SkillKind {
        SKILLS[self.index].kind
    }

    /// 在**当前可负担**的技能之间循环。
    ///
    /// 只循环可负担项，玩家因此不会把选择停在按不出来的技能上。
    pub fn cycle(&mut self, forward: bool, affordable: &[usize]) {
        if affordable.len() < 2 {
            return;
        }
        let position = affordable
            .iter()
            .position(|index| *index == self.index)
            .unwrap_or(0);
        let len = affordable.len() as isize;
        let step = if forward { 1 } else { -1 };
        let next = ((position as isize + step).rem_euclid(len)) as usize;
        self.index = affordable[next];
    }
}

/// 选中第 `index` 个技能（数字键 `1`~`4`）。
///
/// 写：[`crate::input`]；消费：[`select_skill_system`]。
#[derive(Message, Debug, Clone, Copy)]
pub struct SelectSkill(pub usize);

/// 循环选择（`Tab` / `Shift+Tab`）。
///
/// 写：[`crate::input`]；消费：[`cycle_skill_system`]。
#[derive(Message, Debug, Clone, Copy)]
pub struct CycleSkill {
    pub forward: bool,
}

/// 释放当前选中的技能（`G` 键 / 鼠标左键点目标）。
///
/// 写：[`crate::input`]；消费：[`use_selected_skill_system`]——按种类派发成
/// `MeleeCommand` / `FireCommand` / `RollCommand`。
///
/// `target_cell`：`None` = 由声明系统挑最近敌人（键盘）；`Some` = 打点中的那一格（鼠标）。
#[derive(Message, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UseSelectedSkill {
    pub target_cell: Option<Cell>,
}

/// 玩家的零件：精力（够不够）+ 决策槽（现在能不能出手）。
///
/// 「谁是玩家」认 [`InputDriven`] 标记，不再满世界 `find(|faction| … == Player)`。
type PlayerUnit<'w, 's> =
    Query<'w, 's, (Entity, &'static Stamina, &'static DecisionSlot), With<InputDriven>>;

/// 选中：只改 [`MenuSelection`]。
pub fn select_skill_system(
    mut requests: MessageReader<SelectSkill>,
    mut selection: ResMut<MenuSelection>,
) {
    for request in requests.read() {
        selection.select(request.0);
    }
}

/// 循环：只在当前可负担的技能之间走。
pub fn cycle_skill_system(
    mut requests: MessageReader<CycleSkill>,
    mut selection: ResMut<MenuSelection>,
    // 选择**随时可做**（忙的时候也能先把下一个选好），因此只读玩家当前的精力
    players: Query<&Stamina, With<InputDriven>>,
) {
    for request in requests.read() {
        let affordable = players
            .iter()
            .next()
            .map(|stamina| super::registry::affordable_indices(stamina.current))
            .unwrap_or_default();
        selection.cycle(request.forward, &affordable);
    }
}

/// 派发：把「当前选中的技能」翻译成对应领域的消息。
///
/// 只做翻译与**可用性判断**（精力是否够）——真正的扣费与行动声明在
/// [`crate::combat`] / [`crate::movement`] 的声明系统里。
#[allow(clippy::too_many_arguments)]
pub fn use_selected_skill_system(
    mut requests: MessageReader<UseSelectedSkill>,
    selection: Res<MenuSelection>,
    players: PlayerUnit<'_, '_>,
    transforms: Query<&Transform>,
    bodies: Query<(&Transform, &Faction)>,
    mut fire_commands: MessageWriter<FireCommand>,
    mut melee_commands: MessageWriter<MeleeCommand>,
    mut roll_commands: MessageWriter<RollCommand>,
    mut blocked: MessageWriter<crate::timeline::ActionBlocked>,
    // 玩家用技能 = 对当前反应窗口的表态（没有窗口时自然被忽略）
    mut answers: MessageWriter<crate::combat::reaction::ReactionAnswer>,
) {
    let Some(request) = requests.read().last().copied() else {
        return;
    };
    // 决策槽不是空的就是"这次输入被拒"（前摇 / 后摇 / 位移中）
    let Some((player, stamina, _)) = players.iter().first_ready(&mut blocked) else {
        return; // 忙（前摇 / 后摇）或没有玩家
    };
    let Some(def) = SKILLS.get(selection.index()) else {
        return;
    };
    if !def.affordable(stamina.current) {
        debug!("技能 {} 精力不足（需要 {}）", def.label(), def.cost);
        blocked.write(crate::timeline::ActionBlocked::NO_ENERGY);
        return;
    }

    // 这一手按下去了，就算对反应窗口表了态（窗口存在与否由消费方判定）。
    // 派发规则（「攻击」→ 近战或火球）由下面按距离决定，这里只报"我用了这一手"。
    answers.write(crate::combat::reaction::ReactionAnswer::Counter(
        def.catalogue_entry()
            .unwrap_or(crate::skills::AbilityId::Melee),
    ));

    match def.kind {
        // 「攻击」按**真实距离**派发：贴脸用近战，否则扔火球
        SkillKind::Attack => {
            let Ok(transform) = transforms.get(player) else {
                return;
            };
            // 鼠标给了目标格就按「玩家到那一格」算距离；键盘路径按最近的敌人算
            let distance = match request.target_cell {
                Some(cell) => {
                    let center = cell.center();
                    let target = Vec3::new(center.x, transform.translation.y, center.y);
                    transform.translation.distance(target)
                }
                None => bodies
                    .iter()
                    .filter(|(_, faction)| **faction != Faction::Player)
                    .map(|(body, _)| body.translation.distance(transform.translation))
                    .min_by(f32::total_cmp)
                    .unwrap_or(f32::MAX),
            };
            if distance <= MELEE_REACH {
                melee_commands.write(MeleeCommand);
            } else {
                fire_commands.write(FireCommand {
                    target_cell: request.target_cell,
                });
            }
        }
        SkillKind::Melee => {
            melee_commands.write(MeleeCommand);
        }
        SkillKind::Fireball => {
            fire_commands.write(FireCommand {
                target_cell: request.target_cell,
            });
        }
        SkillKind::Roll => {
            // 与菜单过滤、声明系统同一条判据（`can_cast`）
            if def.affordable(stamina.current) {
                roll_commands.write(RollCommand);
            }
        }
    }
}

/// 菜单里显示的一行（HUD 用）：`>1:attack (2)`。
pub fn skill_line(index: usize, selected: usize) -> String {
    let def = &SKILLS[index];
    let marker = if index == selected { ">" } else { " " };
    format!("{marker}{}:{} ({})", index + 1, def.label(), def.cost)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_only_accepts_valid_indices() {
        let mut selection = MenuSelection::default();
        assert_eq!(selection.selected(), SkillKind::Attack);
        selection.select(2);
        assert_eq!(selection.selected(), SkillKind::Fireball);
        selection.select(99);
        assert_eq!(
            selection.selected(),
            SkillKind::Fireball,
            "越界选择应当被忽略"
        );
    }

    #[test]
    fn cycling_skips_unaffordable_skills() {
        let mut selection = MenuSelection::default();
        // 只有一个可负担项时不该挪动选择
        selection.cycle(true, &[1]);
        assert_eq!(selection.index(), 0);

        selection.select(1);
        selection.cycle(true, &[1, 3]);
        assert_eq!(selection.index(), 3, "向下一个可负担项循环");
        selection.cycle(true, &[1, 3]);
        assert_eq!(selection.index(), 1, "循环回到开头");
        selection.cycle(false, &[1, 3]);
        assert_eq!(selection.index(), 3, "反向循环");
    }

    #[test]
    fn skill_line_marks_the_selection() {
        assert!(skill_line(0, 0).starts_with('>'));
        assert!(skill_line(1, 0).starts_with(' '));
        assert!(skill_line(2, 0).contains("fireball"));
    }

    #[test]
    fn dispatch_boundary_is_three_quarters_of_a_cell() {
        assert_eq!(MELEE_REACH, 1.5);
    }
}
