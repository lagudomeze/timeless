//! # 菜单领域：能力组件 + 技能表 + 输入系统（一个文件）
//!
//! 行动本体是组件（`Attack` / `Move` / `Roll` / `Fireball`，见 combat/movement），
//! 菜单只做三件事：
//! - 用能力标记（`Can*`）查询实体「可用哪些行动」，动态生成可选项；
//! - 用 `SKILLS` 展示表（标签 / 消耗 / 插入工厂）把选择转换为行动组件；
//! - 键盘 / 面板消息驱动决策与反应阶段。

use bevy::prelude::*;

use timeless_domain::grid::GridPos;

use crate::combat::{
    Attack, BattleLog, Enemy, Parry, ParryExecuted, Player, RollExecuted, Stamina,
};
use crate::display::map::GRID_SIZE;
use crate::movement::{Fireball, Move, Position, Roll};
use crate::timeline::{TimeLineState, TurnPhase};

// ─────────────────────────── 能力组件 ───────────────────────────

/// 可执行行动的能力标记（挂在玩家实体上，驱动决策菜单的可选项）
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanAttack;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanMove;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanRoll;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanFireball;

// ─────────────────────────── 技能 / 反应表（UI 展示 + 插入工厂） ───────────────────────────

/// 技能定义：只含 UI 展示元数据与「挂载行动组件」的工厂。
/// 不存储任何游戏状态——行动本体是组件，本表只是选项的顺序 / 标签 / 消耗。
pub(crate) struct SkillDef {
    pub label: &'static str,
    pub cost: u32,
    /// 把选择转换为行动组件（`Move` 的 target 由 WASD 实时传入）
    pub insert: fn(&mut Commands, Entity, GridPos, Option<Position>),
}

fn insert_attack(
    commands: &mut Commands,
    entity: Entity,
    _enemy: GridPos,
    _target: Option<Position>,
) {
    commands.entity(entity).insert(Attack);
}

fn insert_move(commands: &mut Commands, entity: Entity, _enemy: GridPos, target: Option<Position>) {
    if let Some(target) = target {
        commands.entity(entity).insert(Move { target });
    }
}

fn insert_roll(commands: &mut Commands, entity: Entity, enemy: GridPos, _target: Option<Position>) {
    commands.entity(entity).insert(Roll { from: enemy });
}

fn insert_fireball(
    commands: &mut Commands,
    entity: Entity,
    enemy: GridPos,
    _target: Option<Position>,
) {
    commands.entity(entity).insert(Fireball {
        target: Position(enemy),
        speed: FIREBALL_SPEED,
        amount: FIREBALL_AMOUNT,
        radius: FIREBALL_RADIUS,
    });
}

/// 火球技能参数（Phase 2.1 将外置到 ActionTemplate 配置）
pub(crate) const FIREBALL_COST: u32 = 2;
const FIREBALL_SPEED: f32 = 4.0;
const FIREBALL_AMOUNT: u32 = 8;
const FIREBALL_RADIUS: u32 = 1;
/// 翻滚取消 / 招架反应消耗
pub(crate) const ROLL_CANCEL_COST: u32 = 2;
pub(crate) const PARRY_COST: u32 = 1;

/// 技能表：固定顺序即 Tab 循环顺序（可用性由能力组件动态过滤）
pub(crate) const SKILLS: [SkillDef; 4] = [
    SkillDef {
        label: "攻击",
        cost: 0,
        insert: insert_attack,
    },
    SkillDef {
        label: "移动",
        cost: 0,
        insert: insert_move,
    },
    SkillDef {
        label: "翻滚",
        cost: 0,
        insert: insert_roll,
    },
    SkillDef {
        label: "火球",
        cost: FIREBALL_COST,
        insert: insert_fireball,
    },
];

/// 反应定义：继续攻击 / 翻滚取消 / 招架
pub(crate) struct ReactionDef {
    pub label: &'static str,
    pub cost: u32,
}

pub(crate) const REACTIONS: [ReactionDef; 3] = [
    ReactionDef {
        label: "继续攻击",
        cost: 0,
    },
    ReactionDef {
        label: "翻滚取消",
        cost: ROLL_CANCEL_COST,
    },
    ReactionDef {
        label: "招架",
        cost: PARRY_COST,
    },
];

/// 按能力组件过滤出可用的技能索引（固定顺序，供菜单 / HUD / 调试面板共用）
pub(crate) fn available_skills(
    can_attack: bool,
    can_move: bool,
    can_roll: bool,
    can_fireball: bool,
) -> Vec<usize> {
    let mut available = Vec::new();
    if can_attack {
        available.push(0);
    }
    if can_move {
        available.push(1);
    }
    if can_roll {
        available.push(2);
    }
    if can_fireball {
        available.push(3);
    }
    available
}

// ─────────────────────────── 状态与消息 ───────────────────────────

/// 菜单选择游标：决策阶段指向「可用技能」下标，反应阶段指向「反应」下标
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MenuSelection {
    pub index: usize,
}

/// 面板选择技能（决策阶段）：payload 为 `SKILLS` 下标
#[derive(Message, Debug, Clone, Copy)]
pub struct SelectSkill(pub usize);

/// 面板提交指令（决策阶段）：等价于按 Space
#[derive(Message, Debug, Clone, Copy)]
pub struct CommitTurn;

/// 面板选择反应（Reaction 阶段）：payload 为 `REACTIONS` 下标
#[derive(Message, Debug, Clone, Copy)]
pub struct ReactionSelect(pub usize);

// ─────────────────────────── 系统 ───────────────────────────

/// 决策输入：玩家（实体 + 当前位置）
type PlayerInputQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Position), (With<Player>, Without<Enemy>)>;

/// 玩家能力查询（四个 `Can*` 标记）
type PlayerCapabilityQuery<'w, 's> = Query<
    'w,
    's,
    (Has<CanAttack>, Has<CanMove>, Has<CanRoll>, Has<CanFireball>),
    (With<Player>, Without<Enemy>),
>;

/// 反应输入：玩家（实体 + 精力）
type PlayerReactionQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static mut Stamina), (With<Player>, Without<Enemy>)>;

/// 决策阶段输入（一切冻结，只等玩家）：
/// - Tab / Shift+Tab 在**能力允许**的技能间循环，选中即插入对应行动组件
/// - WASD / 方向键：直接插入 `Move` 行动（支持斜向）
/// - 面板 `SelectSkill` 消息 / `CommitTurn` 消息 或 Space 键提交
///
/// 参数较多：键盘 + 状态 + 游标 + 命令 + 玩家/敌人/能力/精力查询 + 消息通道，属合理边界。
#[allow(clippy::too_many_arguments)]
pub fn input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut tl: ResMut<TimeLineState>,
    mut menu: ResMut<MenuSelection>,
    mut commands: Commands,
    player_q: PlayerInputQuery<'_, '_>,
    capability_q: PlayerCapabilityQuery<'_, '_>,
    enemy_q: Query<&Position, (With<Enemy>, Without<Player>)>,
    player_attack_q: Query<&Attack, (With<Player>, Without<Enemy>)>,
    player_move_q: Query<&Move, (With<Player>, Without<Enemy>)>,
    enemy_attack_q: Query<&Attack, (With<Enemy>, Without<Player>)>,
    mut stamina_q: Query<&mut Stamina, (With<Player>, Without<Enemy>)>,
    mut log: ResMut<BattleLog>,
    mut ev_select: MessageReader<SelectSkill>,
    mut ev_commit: MessageReader<CommitTurn>,
) {
    if tl.phase != TurnPhase::Decision {
        return;
    }
    let Ok((entity, pos)) = player_q.single() else {
        return;
    };
    let Ok((can_attack, can_move, can_roll, can_fireball)) = capability_q.single() else {
        return;
    };
    let available = available_skills(can_attack, can_move, can_roll, can_fireball);
    if available.is_empty() {
        return;
    }
    let Ok(enemy_pos) = enemy_q.single() else {
        return;
    };

    // Tab / Shift+Tab 循环切换可用技能（仅 Tab 键，方向键让位移动）
    let len = available.len();
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if keys.just_pressed(KeyCode::Tab) {
        menu.index = if shift {
            (menu.index + len - 1) % len
        } else {
            (menu.index + 1) % len
        };
        let skill = available[menu.index];
        (SKILLS[skill].insert)(&mut commands, entity, enemy_pos.0, None);
        info!("[决策] 选中 {}（Tab）", SKILLS[skill].label);
        log.push(format!("[决策] 选中 {}", SKILLS[skill].label));
    }

    // WASD / 方向键：直接插入移动行动。以本帧按下为触发，叠加当前按住的所有方向，
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
                (SKILLS[1].insert)(&mut commands, entity, enemy_pos.0, Some(target));
                if let Some(i) = available.iter().position(|&s| s == 1) {
                    menu.index = i;
                }
                info!("[决策] 移动 → ({},{})", target.0.x, target.0.y);
            }
        }
    }

    // 面板选择技能（同步游标并插入行动组件）
    if let Some(sel) = ev_select.read().next()
        && let Some(i) = available.iter().position(|&s| s == sel.0)
    {
        menu.index = i;
        (SKILLS[sel.0].insert)(&mut commands, entity, enemy_pos.0, None);
        info!("[决策] 玩家行动设为 {}（面板）", SKILLS[sel.0].label);
    }

    // 提交（Space / 面板按钮）
    let commit = keys.just_pressed(KeyCode::Space) || ev_commit.read().next().is_some();
    if !commit {
        return;
    }
    let skill = available[menu.index];
    let skill_def = &SKILLS[skill];
    // 移动行动必须先指定方向（WASD/方向键），防止空提交白费一回合
    if skill == 1 && player_move_q.get(entity).is_err() {
        info!("[决策] 移动行动需先用 WASD/方向键 指定方向");
        return;
    }
    // 有消耗的技能（当前仅火球）提交即扣精力
    if skill_def.cost > 0
        && let Ok(mut stamina) = stamina_q.get_mut(entity)
        && !stamina.try_spend(skill_def.cost)
    {
        info!(
            "[决策] 精力不足（需 {}，当前 {}）—— 无法施放",
            skill_def.cost, stamina.current
        );
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
    log.push(format!("[提交] 玩家行动：{}", skill_def.label));
}

/// 反应阶段（玩家独有特权，一切冻结）：
/// - Tab / Shift+Tab 导航反应列表（继续攻击 / 翻滚取消 / 招架）
/// - Q 键快捷翻滚取消；Space 执行当前选中项
/// - 选择后进入 Resolving 瞬时结算
#[allow(clippy::too_many_arguments)]
pub fn reaction_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut tl: ResMut<TimeLineState>,
    mut menu: ResMut<MenuSelection>,
    mut commands: Commands,
    mut player_q: PlayerReactionQuery<'_, '_>,
    enemy_q: Query<&Position, (With<Enemy>, Without<Player>)>,
    mut log: ResMut<BattleLog>,
    mut ev_roll: MessageWriter<RollExecuted>,
    mut ev_parry: MessageWriter<ParryExecuted>,
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

    // Tab / Shift+Tab 导航反应列表
    let len = REACTIONS.len();
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if keys.just_pressed(KeyCode::Tab) {
        menu.index = if shift {
            (menu.index + len - 1) % len
        } else {
            (menu.index + 1) % len
        };
        info!("[反应] 选中 {}", REACTIONS[menu.index].label);
    }

    // 选择：Q 快捷翻滚取消 / Space 执行选中项 / 面板消息
    let choice = if keys.just_pressed(KeyCode::KeyQ) {
        Some(1)
    } else if keys.just_pressed(KeyCode::Space) {
        Some(menu.index)
    } else {
        ev_reaction.read().next().map(|m| m.0)
    };
    let Some(choice) = choice else {
        return;
    };

    match choice {
        0 => {
            tl.phase = TurnPhase::Resolving;
            info!("[反应] 继续攻击");
        }
        1 => {
            // 翻滚取消：消耗精力，中断攻击转为翻滚（位移 + 闪避）
            if !stamina.try_spend(ROLL_CANCEL_COST) {
                info!(
                    "[翻滚取消] 精力不足（需 {ROLL_CANCEL_COST}，当前 {}）—— 仍在反应阶段",
                    stamina.current
                );
                return; // 留在 Reaction，等待再次选择
            }
            commands
                .entity(entity)
                .remove::<Attack>()
                .insert(Roll { from: enemy_pos.0 });
            tl.phase = TurnPhase::Resolving;
            ev_roll.write(RollExecuted { entity });
            log.push("[翻滚取消] 攻击中断，本回合转为翻滚".to_string());
            info!(
                "[翻滚取消] 攻击中断！消耗 {ROLL_CANCEL_COST} 精力（剩余 {}），本回合转为翻滚后退",
                stamina.current
            );
        }
        2 => {
            // 招架：消耗精力，中断攻击转为格挡（免疫本次攻击 + 反制一半伤害）
            if !stamina.try_spend(PARRY_COST) {
                info!(
                    "[招架] 精力不足（需 {PARRY_COST}，当前 {}）—— 仍在反应阶段",
                    stamina.current
                );
                return; // 留在 Reaction，等待再次选择
            }
            commands.entity(entity).remove::<Attack>().insert(Parry);
            tl.phase = TurnPhase::Resolving;
            ev_parry.write(ParryExecuted { entity });
            log.push(format!(
                "[招架] 攻击中断，消耗 {PARRY_COST} 精力，本回合转为招架姿态"
            ));
            info!(
                "[招架] 攻击中断！消耗 {PARRY_COST} 精力（剩余 {}），本回合转为招架姿态",
                stamina.current
            );
        }
        _ => {}
    }
}
