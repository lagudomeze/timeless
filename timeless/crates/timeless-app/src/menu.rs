//! # 菜单领域：组件 + 消息 + 系统（一个文件）
//!
//! 暂停菜单（决策 / 反应）的游标状态、行动选项、面板交互消息与键盘输入系统。
//! `Action` 枚举只作 UI 层的选项标识；实际回合意图以组件形式挂到单位上：
//! - 攻击 → `AttackIntent`
//! - 移动 → `MoveIntent { target }`（见 `movement`）
//! - 翻滚 → `RetreatIntent` + `DodgeActive`（见 `movement` / `combat`）

use bevy::prelude::*;

use timeless_domain::grid::GridPos;

use crate::combat::{AttackIntent, DodgeActive, Enemy, Player, RollExecuted, Stamina};
use crate::display::map::GRID_SIZE;
use crate::movement::{MoveIntent, Position, RetreatIntent};
use crate::timeline::{TimeLineState, TurnPhase};

// ─────────────────────────── 状态 ───────────────────────────

/// 反应选项（Reaction 阶段）：继续攻击 / 翻滚取消
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactionChoice {
    Continue,
    RollCancel,
}

/// 行动选项（UI 层标识；提交时转换为意图组件）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Action {
    #[default]
    None,
    Attack,
    Move,
    Roll,
}

/// 菜单选择游标（Tab 循环导航行动/反应列表；index 指向当前选项）
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MenuSelection {
    pub index: usize,
}

/// 决策阶段可用行动（Tab 循环切换；WASD/方向键直接生成移动行动）
pub(crate) const DECISION_OPTIONS: [Action; 3] = [Action::Attack, Action::Move, Action::Roll];
/// 反应阶段可用反应（Tab 循环切换）
pub(crate) const REACTION_OPTIONS: [ReactionChoice; 2] =
    [ReactionChoice::Continue, ReactionChoice::RollCancel];

// ─────────────────────────── 消息 ───────────────────────────

/// 行动已提交（Space 按下时广播，供日志 / 表现订阅）
#[derive(Message, Debug, Clone, Copy)]
pub struct ActionSubmitted {
    pub entity: Entity,
    pub action: Action,
}

/// 面板选择行动（决策阶段）：设置玩家本回合意图
#[derive(Message, Debug, Clone, Copy)]
pub struct SelectAction(pub Action);

/// 面板提交指令（决策阶段）：等价于按 Space
#[derive(Message, Debug, Clone, Copy)]
pub struct CommitTurn;

/// 面板选择反应（Reaction 阶段）
#[derive(Message, Debug, Clone, Copy)]
pub struct ReactionSelect(pub ReactionChoice);

// ─────────────────────────── 系统 ───────────────────────────

/// 决策输入：玩家（实体 + 当前位置）
type PlayerInputQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Position), (With<Player>, Without<Enemy>)>;

/// 反应输入：玩家（实体 + 精力）
type PlayerReactionQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static mut Stamina), (With<Player>, Without<Enemy>)>;

/// 决策阶段输入（一切冻结，只等玩家）：
/// - Tab / Shift+Tab 循环切换行动（攻击 / 移动 / 翻滚），选中项写入意图组件
/// - WASD / 方向键：生成「移动」意图（支持同时按两个方向 → 斜向一格）
/// - 面板 `SelectAction` 消息：设置玩家意图
/// - 面板 `CommitTurn` 消息 或 Space 键：提交 → 威胁检测决定进入 Reaction 或 Resolving
///
/// 参数较多：键盘 + 状态 + 游标 + 命令 + 玩家/敌人查询 + 3 个消息通道，属合理边界。
#[allow(clippy::too_many_arguments)]
pub fn input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut tl: ResMut<TimeLineState>,
    mut menu: ResMut<MenuSelection>,
    mut commands: Commands,
    player_q: PlayerInputQuery<'_, '_>,
    enemy_q: Query<&Position, (With<Enemy>, Without<Player>)>,
    player_attack_q: Query<&AttackIntent, (With<Player>, Without<Enemy>)>,
    player_move_q: Query<&MoveIntent, (With<Player>, Without<Enemy>)>,
    enemy_attack_q: Query<&AttackIntent, (With<Enemy>, Without<Player>)>,
    mut ev_submit: MessageWriter<ActionSubmitted>,
    mut ev_select: MessageReader<SelectAction>,
    mut ev_commit: MessageReader<CommitTurn>,
) {
    if tl.phase != TurnPhase::Decision {
        return;
    }
    let Ok((entity, pos)) = player_q.single() else {
        return;
    };
    let Ok(enemy_pos) = enemy_q.single() else {
        return;
    };

    // Tab / Shift+Tab 循环切换行动（仅 Tab 键，方向键让位移动）
    let len = DECISION_OPTIONS.len();
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if keys.just_pressed(KeyCode::Tab) {
        menu.index = if shift {
            (menu.index + len - 1) % len
        } else {
            (menu.index + 1) % len
        };
        set_pending_action(
            &mut commands,
            entity,
            DECISION_OPTIONS[menu.index],
            None,
            enemy_pos.0,
        );
        info!("[决策] 选中 {:?}（Tab）", DECISION_OPTIONS[menu.index]);
    }

    // WASD / 方向键：生成移动意图。以本帧按下为触发，叠加当前按住的所有方向，
    // 因此「W+A 同按」或「先 W 后 A」都会得到左上斜向目标。
    let dir_triggered = keys.just_pressed(KeyCode::KeyW)
        || keys.just_pressed(KeyCode::ArrowUp)
        || keys.just_pressed(KeyCode::KeyS)
        || keys.just_pressed(KeyCode::ArrowDown)
        || keys.just_pressed(KeyCode::KeyA)
        || keys.just_pressed(KeyCode::ArrowLeft)
        || keys.just_pressed(KeyCode::KeyD)
        || keys.just_pressed(KeyCode::ArrowRight);
    if dir_triggered {
        let mut dx = 0;
        let mut dy = 0;
        if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
            dy -= 1;
        }
        if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
            dy += 1;
        }
        if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
            dx -= 1;
        }
        if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
            dx += 1;
        }
        if dx != 0 || dy != 0 {
            let target = Position::new(
                (pos.0.x + dx).clamp(0, GRID_SIZE - 1),
                (pos.0.y + dy).clamp(0, GRID_SIZE - 1),
            );
            if target != *pos {
                set_pending_action(
                    &mut commands,
                    entity,
                    Action::Move,
                    Some(target),
                    enemy_pos.0,
                );
                if let Some(i) = DECISION_OPTIONS.iter().position(|a| *a == Action::Move) {
                    menu.index = i;
                }
                info!("[决策] 移动 → ({},{})", target.0.x, target.0.y);
            }
        }
    }

    // 面板选择行动（同步游标与意图）
    if let Some(sel) = ev_select.read().next() {
        set_pending_action(&mut commands, entity, sel.0, None, enemy_pos.0);
        if let Some(i) = DECISION_OPTIONS.iter().position(|a| *a == sel.0) {
            menu.index = i;
        }
        info!("[决策] 玩家行动设为 {:?}（面板）", sel.0);
    }

    // 提交（Space / 面板按钮）
    let commit = keys.just_pressed(KeyCode::Space) || ev_commit.read().next().is_some();
    if !commit {
        return;
    }
    // 移动行动必须先指定方向（WASD/方向键），防止空提交白费一回合
    if DECISION_OPTIONS[menu.index] == Action::Move && player_move_q.get(entity).is_err() {
        info!("[决策] 移动行动需先用 WASD/方向键 指定方向");
        return;
    }

    // 威胁检测：双方本回合都将攻击 → 进入 Reaction 暂停等待
    let player_attacks = player_attack_q.get(entity).is_ok();
    let enemy_attacks = enemy_attack_q.single().is_ok();
    tl.phase = if player_attacks && enemy_attacks {
        info!("[威胁] 双方都将攻击 —— 进入反应阶段（暂停等待选择）");
        // 锁定当帧：提交用的 Space/Q 不能被 reaction_system 同帧误消费
        tl.reaction_input_locked = true;
        menu.index = 0; // 反应列表从头开始
        TurnPhase::Reaction
    } else {
        TurnPhase::Resolving
    };
    ev_submit.write(ActionSubmitted {
        entity,
        action: DECISION_OPTIONS[menu.index],
    });
}

/// 反应阶段（玩家独有特权，一切冻结）：
/// - Tab / Shift+Tab 导航反应列表（Continue / Roll-Cancel）
/// - Q 键：翻滚取消（精力×2；不足则留在此阶段）
/// - Space 键：执行当前选中的反应
///
/// 选择后进入 Resolving 瞬时结算。
#[allow(clippy::too_many_arguments)]
pub fn reaction_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut tl: ResMut<TimeLineState>,
    mut menu: ResMut<MenuSelection>,
    mut commands: Commands,
    mut player_q: PlayerReactionQuery<'_, '_>,
    enemy_q: Query<&Position, (With<Enemy>, Without<Player>)>,
    mut ev_roll: MessageWriter<RollExecuted>,
    mut ev_reaction: MessageReader<ReactionSelect>,
) {
    if tl.phase != TurnPhase::Reaction {
        return;
    }
    // 进入 Reaction 的当帧：解锁并跳过，等玩家下一帧再做选择
    if tl.reaction_input_locked {
        tl.reaction_input_locked = false;
        return;
    }
    let Ok((entity, mut stamina)) = player_q.single_mut() else {
        return;
    };
    let Ok(enemy_pos) = enemy_q.single() else {
        return;
    };

    // Tab / Shift+Tab 导航反应列表（Tab = 下一项，Shift+Tab = 上一项）
    let len = REACTION_OPTIONS.len();
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if keys.just_pressed(KeyCode::Tab) && !shift {
        menu.index = (menu.index + 1) % len;
        info!("[反应] 选中 {:?}", REACTION_OPTIONS[menu.index]);
    }
    if keys.just_pressed(KeyCode::Tab) && shift {
        menu.index = (menu.index + len - 1) % len;
        info!("[反应] 选中 {:?}", REACTION_OPTIONS[menu.index]);
    }

    // 选择：Q 快捷翻滚取消 / Space 执行选中项 / 面板消息
    let choice = if keys.just_pressed(KeyCode::KeyQ) {
        menu.index = REACTION_OPTIONS
            .iter()
            .position(|c| *c == ReactionChoice::RollCancel)
            .unwrap_or(1);
        Some(ReactionChoice::RollCancel)
    } else if keys.just_pressed(KeyCode::Space) {
        Some(REACTION_OPTIONS[menu.index])
    } else {
        ev_reaction.read().next().map(|m| m.0)
    };
    let Some(choice) = choice else {
        return;
    };

    match choice {
        ReactionChoice::Continue => {
            tl.phase = TurnPhase::Resolving;
            info!("[反应] 继续攻击");
        }
        ReactionChoice::RollCancel => {
            // 取消惩罚：精力×2（基础翻滚 1 + 取消附加 1）
            if !stamina.try_spend(2) {
                info!(
                    "[翻滚取消] 精力不足（需 2，当前 {}）—— 仍在反应阶段，可改选继续攻击",
                    stamina.current
                );
                return; // 留在 Reaction，等待再次选择
            }
            commands
                .entity(entity)
                .remove::<AttackIntent>()
                .insert(RetreatIntent { from: enemy_pos.0 })
                .insert(DodgeActive);
            tl.phase = TurnPhase::Resolving;
            ev_roll.write(RollExecuted { entity });
            info!(
                "[翻滚取消] 攻击中断！消耗 2 精力（剩余 {}），本回合转为翻滚后退",
                stamina.current
            );
        }
    }
}

/// 将玩家当前选择的行动转换为意图组件（先清除旧意图，再挂载新意图）。
/// `Move` 未指定方向时不挂任何意图（提交时会被空提交检查拦截）。
fn set_pending_action(
    commands: &mut Commands,
    entity: Entity,
    action: Action,
    target: Option<Position>,
    enemy_pos: GridPos,
) {
    commands
        .entity(entity)
        .remove::<AttackIntent>()
        .remove::<MoveIntent>()
        .remove::<RetreatIntent>()
        .remove::<DodgeActive>();
    match action {
        Action::Attack => {
            commands.entity(entity).insert(AttackIntent);
        }
        Action::Move => {
            if let Some(target) = target {
                commands.entity(entity).insert(MoveIntent { target });
            }
        }
        Action::Roll => {
            commands
                .entity(entity)
                .insert(RetreatIntent { from: enemy_pos })
                .insert(DodgeActive);
        }
        Action::None => {}
    }
}
