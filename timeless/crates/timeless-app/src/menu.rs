//! # 菜单领域：能力组件 + 技能表 + 动作声明（一个文件）
//!
//! 行动本体是动作实体（载荷组件 `Attack` / `MoveTo` / `Roll` / `Fireball` /
//! `Parry` + `ScheduledAction`，经时间线调度），菜单只做三件事：
//! - 用能力标记（`Can*`）查询实体「可用哪些行动」，动态生成可选项；
//! - 用 `SKILLS` 展示表（标签 / 消耗 / 动作种类）把选择转换成 Declared 动作实体；
//! - 键盘 / 面板消息驱动决策与反应阶段。
//!
//! 输入按「键盘只翻译按键 → 消息，状态修改下沉到单一职责系统」组织：
//!
//! ```text
//! 决策阶段：
//!   decision_keyboard_system ──CycleSkill──▶ select_skill_system（声明动作实体）
//!          │  ──MoveInput────▶ movement::move_input_system（声明 MoveTo）
//!          │  ──CommitTurn───▶ commit_system（校验 + 扣费）──TurnCommitted──▶ timeline::phase_advance_system
//! 反应阶段：
//!   reaction_keyboard_system ──ReactionSelect──▶ reaction_execution_system（翻滚取消 / 招架）
//! ```
//!
//! 面板（debug.rs）直接写 `SelectSkill` / `CommitTurn` / `ReactionSelect`，
//! 与键盘共用同一消费端；`MoveInput` / `TurnCommitted` 分别由消费端所在的
//! movement / timeline 领域定义，模块间仅通过消息耦合。

use bevy::prelude::*;
use bevy::time::Virtual;

use crate::combat::{
    Attack, AttackFrame, AttackRange, BattleLog, Damage, Enemy, Fireball, Impact, Parry, Player,
    Stamina,
};
use crate::movement::{MoveInput, Position, Roll};
use crate::timeline::{
    Declared, Pending, ScheduledAction, TICK_MS, TimeLineState, TurnCommitted, TurnPhase,
    despawn_declared_for,
};

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

// ─────────────────────────── 技能 / 反应表（UI 展示 + 动作声明） ───────────────────────────

/// 技能种类：决定声明哪种动作载荷（声明时由系统查询并快照玩家属性）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SkillKind {
    Attack,
    Move,
    Roll,
    Fireball,
}

/// 技能定义：只含 UI 展示元数据与动作种类，不存储任何游戏状态
pub(crate) struct SkillDef {
    pub label: &'static str,
    pub cost: u32,
    pub kind: SkillKind,
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
        kind: SkillKind::Attack,
    },
    SkillDef {
        label: "移动",
        cost: 0,
        kind: SkillKind::Move,
    },
    SkillDef {
        label: "翻滚",
        cost: 0,
        kind: SkillKind::Roll,
    },
    SkillDef {
        label: "火球",
        cost: FIREBALL_COST,
        kind: SkillKind::Fireball,
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

/// 面板选中技能（决策阶段）：payload 为 `SKILLS` 下标
#[derive(Message, Debug, Clone, Copy)]
pub struct SelectSkill(pub usize);

/// 键盘 Tab 循环选择（决策阶段）：`forward` 为 false 时反向（Shift+Tab）
#[derive(Message, Debug, Clone, Copy)]
pub struct CycleSkill {
    pub forward: bool,
}

/// 面板 / 键盘提交指令（决策阶段）：等价于按 Space
#[derive(Message, Debug, Clone, Copy)]
pub struct CommitTurn;

/// 面板 / 键盘选择反应（Reaction 阶段）：payload 为 `REACTIONS` 下标
#[derive(Message, Debug, Clone, Copy)]
pub struct ReactionSelect(pub usize);

// ─────────────────────────── 系统 ───────────────────────────

/// 玩家实体查询（决策阶段）
type PlayerEntityQuery<'w, 's> = Query<'w, 's, Entity, (With<Player>, Without<Enemy>)>;

/// 玩家攻击属性快照（声明 Attack 动作时使用）
type PlayerStatsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Damage,
        &'static AttackRange,
        &'static Impact,
        &'static AttackFrame,
    ),
    (With<Player>, Without<Enemy>),
>;

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

/// 决策阶段键盘输入（只翻译按键 → 消息，不直接改状态）：
/// - Tab / Shift+Tab → `CycleSkill`（循环逻辑在 `select_skill_system`）
/// - WASD / 方向键 → `MoveInput`（按住叠加支持斜向；落格逻辑在 movement）
/// - Space → `CommitTurn`（校验 / 扣费在 `commit_system`）
pub fn decision_keyboard_system(
    keys: Res<ButtonInput<KeyCode>>,
    tl: Res<TimeLineState>,
    mut ev_cycle: MessageWriter<CycleSkill>,
    mut ev_move: MessageWriter<MoveInput>,
    mut ev_commit: MessageWriter<CommitTurn>,
) {
    if tl.phase != TurnPhase::Decision {
        return;
    }

    // Tab / Shift+Tab 循环切换可用技能（仅 Tab 键，方向键让位移动）
    if keys.just_pressed(KeyCode::Tab) {
        let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        ev_cycle.write(CycleSkill { forward: !shift });
    }

    // WASD / 方向键：以本帧按下为触发，叠加当前按住的所有方向，
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
            ev_move.write(MoveInput { dx, dy });
        }
    }

    if keys.just_pressed(KeyCode::Space) {
        ev_commit.write(CommitTurn);
    }
}

/// 把技能选择转换成 Declared 动作实体（替换玩家已声明的其他动作）
fn declare_action(
    commands: &mut Commands,
    kind: SkillKind,
    player: Entity,
    enemy: Entity,
    enemy_pos: IVec2,
    stats: (u32, u32, u32, u32), // damage, range, impact, frame
) {
    match kind {
        SkillKind::Attack => {
            commands.spawn_scene(bsn! {
                Attack {
                    target: {enemy},
                    damage: {stats.0},
                    range: {stats.1},
                    impact: {stats.2},
                }
                ScheduledAction {
                    execute_at: 0,
                    cast_duration: {stats.3 as u64 * TICK_MS},
                    actor: {player},
                }
                Declared
            });
        }
        // 移动方向由 WASD 经 `MoveInput` 实时指定（见 movement::move_input_system）
        SkillKind::Move => {}
        SkillKind::Roll => {
            commands.spawn_scene(bsn! {
                Roll { from: {enemy_pos} }
                ScheduledAction { execute_at: 0, cast_duration: 0, actor: {player} }
                Declared
            });
        }
        SkillKind::Fireball => {
            commands.spawn_scene(bsn! {
                Fireball {
                    target: {enemy_pos},
                    speed: {FIREBALL_SPEED},
                    amount: {FIREBALL_AMOUNT},
                    radius: {FIREBALL_RADIUS},
                }
                ScheduledAction { execute_at: 0, cast_duration: 0, actor: {player} }
                Declared
            });
        }
    }
}

/// 技能选择（消费键盘 Tab 与面板 `SelectSkill`）：同步游标并声明动作实体
#[allow(clippy::too_many_arguments)]
pub fn select_skill_system(
    tl: Res<TimeLineState>,
    mut menu: ResMut<MenuSelection>,
    mut commands: Commands,
    player_q: PlayerStatsQuery<'_, '_>,
    capability_q: PlayerCapabilityQuery<'_, '_>,
    enemy_q: Query<(Entity, &Position), With<Enemy>>,
    declared_q: Query<(Entity, &ScheduledAction), With<Declared>>,
    mut ev_cycle: MessageReader<CycleSkill>,
    mut ev_select: MessageReader<SelectSkill>,
) {
    if tl.phase != TurnPhase::Decision {
        return;
    }
    let Ok((player, damage, range, impact, frame)) = player_q.single() else {
        return;
    };
    let Ok((can_attack, can_move, can_roll, can_fireball)) = capability_q.single() else {
        return;
    };
    let available = available_skills(can_attack, can_move, can_roll, can_fireball);
    if available.is_empty() {
        return;
    }
    let Ok((enemy, enemy_pos)) = enemy_q.single() else {
        return;
    };
    let stats = (damage.0, range.0, impact.0, frame.0);

    // Tab / Shift+Tab 循环切换可用技能
    let len = available.len();
    for cycle in ev_cycle.read() {
        menu.index = if cycle.forward {
            (menu.index + 1) % len
        } else {
            (menu.index + len - 1) % len
        };
        let skill = available[menu.index];
        despawn_declared_for(&mut commands, player, &declared_q);
        declare_action(
            &mut commands,
            SKILLS[skill].kind,
            player,
            enemy,
            enemy_pos.0,
            stats,
        );
        info!("[决策] 选中 {}（Tab）", SKILLS[skill].label);
    }

    // 面板选择技能（同步游标并声明动作）
    for sel in ev_select.read() {
        let Some(i) = available.iter().position(|&s| s == sel.0) else {
            continue;
        };
        menu.index = i;
        despawn_declared_for(&mut commands, player, &declared_q);
        declare_action(
            &mut commands,
            SKILLS[sel.0].kind,
            player,
            enemy,
            enemy_pos.0,
            stats,
        );
        info!("[决策] 玩家行动设为 {}（面板）", SKILLS[sel.0].label);
    }
}

/// 提交回合（消费 Space 与面板 `CommitTurn`）：
/// 校验动作已声明 / 移动方向已指定、扣除技能消耗，成功后广播 `TurnCommitted`。
#[allow(clippy::too_many_arguments)]
pub fn commit_system(
    tl: Res<TimeLineState>,
    menu: Res<MenuSelection>,
    player_q: PlayerEntityQuery<'_, '_>,
    capability_q: PlayerCapabilityQuery<'_, '_>,
    declared_q: Query<&ScheduledAction, With<Declared>>,
    move_declared_q: Query<(&ScheduledAction, &crate::movement::MoveTo), With<Declared>>,
    mut stamina_q: Query<&mut Stamina, (With<Player>, Without<Enemy>)>,
    mut log: ResMut<BattleLog>,
    mut ev_commit: MessageReader<CommitTurn>,
    mut ev_committed: MessageWriter<TurnCommitted>,
) {
    if tl.phase != TurnPhase::Decision {
        return;
    }
    if ev_commit.read().next().is_none() {
        return;
    }
    let Ok(player) = player_q.single() else {
        return;
    };
    let Ok((can_attack, can_move, can_roll, can_fireball)) = capability_q.single() else {
        return;
    };
    let available = available_skills(can_attack, can_move, can_roll, can_fireball);
    if available.is_empty() {
        return;
    }

    let skill = available[menu.index];
    let skill_def = &SKILLS[skill];
    let has_declared = declared_q.iter().any(|s| s.actor == player);
    let has_move = move_declared_q.iter().any(|(s, _)| s.actor == player);
    // 移动行动必须先指定方向（WASD/方向键），防止空提交白费一回合
    if skill_def.kind == SkillKind::Move && !has_move {
        info!("[决策] 移动行动需先用 WASD/方向键 指定方向");
        return;
    }
    // 其他动作必须在选择时已声明
    if skill_def.kind != SkillKind::Move && !has_declared {
        info!("[决策] 请先选择行动（Tab / 面板）");
        return;
    }
    // 有消耗的技能（当前仅火球）提交即扣精力
    if skill_def.cost > 0
        && let Ok(mut stamina) = stamina_q.get_mut(player)
        && !stamina.try_spend(skill_def.cost)
    {
        info!(
            "[决策] 精力不足（需 {}，当前 {}）—— 无法施放",
            skill_def.cost, stamina.current
        );
        return;
    }

    ev_committed.write(TurnCommitted);
    log.push(format!("[提交] 玩家行动：{}", skill_def.label));
}

/// 反应阶段键盘输入（只翻译按键 → 消息 / 游标导航）：
/// - Tab / Shift+Tab 导航反应列表（继续攻击 / 翻滚取消 / 招架）
/// - Q 键快捷翻滚取消；Space 执行当前选中项
///
/// 反应窗口期间时间冻结（`Time<Virtual>` 暂停），不存在同帧误消费问题。
pub fn reaction_keyboard_system(
    keys: Res<ButtonInput<KeyCode>>,
    tl: Res<TimeLineState>,
    mut menu: ResMut<MenuSelection>,
    mut ev_reaction: MessageWriter<ReactionSelect>,
) {
    if tl.phase != TurnPhase::Reaction {
        return;
    }

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

    // 选择：Q 快捷翻滚取消 / Space 执行选中项（面板消息由执行系统直接消费）
    let choice = if keys.just_pressed(KeyCode::KeyQ) {
        Some(1)
    } else if keys.just_pressed(KeyCode::Space) {
        Some(menu.index)
    } else {
        None
    };
    if let Some(choice) = choice {
        ev_reaction.write(ReactionSelect(choice));
    }
}

/// 反应执行（消费键盘 Q/Space 与面板 `ReactionSelect`）：
/// - 0 继续攻击 → 恢复 Resolving（时间放行）
/// - 1 翻滚取消：消耗精力，撤销玩家攻击，声明立即执行的 `Roll`（位移 + 闪避）
/// - 2 招架：消耗精力，声明绑定到敌人攻击实体的 `Parry`
#[allow(clippy::too_many_arguments)]
pub fn reaction_execution_system(
    time: Res<Time<Virtual>>,
    mut tl: ResMut<TimeLineState>,
    mut commands: Commands,
    mut player_q: PlayerReactionQuery<'_, '_>,
    enemy_q: Query<(Entity, &Position), With<Enemy>>,
    pending_attacks: Query<(Entity, &ScheduledAction, &Attack), With<Pending>>,
    mut log: ResMut<BattleLog>,
    mut ev_reaction: MessageReader<ReactionSelect>,
) {
    if tl.phase != TurnPhase::Reaction {
        return;
    }
    let Some(choice) = ev_reaction.read().next() else {
        return;
    };
    let Ok((player, mut stamina)) = player_q.single_mut() else {
        return;
    };
    let Ok((enemy, enemy_pos)) = enemy_q.single() else {
        return;
    };

    match choice.0 {
        0 => {
            tl.phase = TurnPhase::Resolving;
            tl.reaction_resolved = true;
            info!("[反应] 继续攻击");
        }
        1 => {
            // 翻滚取消：消耗精力，撤销攻击转为翻滚（位移 + 闪避）
            if !stamina.try_spend(ROLL_CANCEL_COST) {
                info!(
                    "[翻滚取消] 精力不足（需 {ROLL_CANCEL_COST}，当前 {}）—— 仍在反应阶段",
                    stamina.current
                );
                return; // 留在 Reaction，等待再次选择
            }
            for (entity, scheduled, _) in &pending_attacks {
                if scheduled.actor == player {
                    commands.entity(entity).despawn();
                }
            }
            let now = time.elapsed().as_millis() as u64;
            commands.spawn_scene(bsn! {
                Roll { from: {enemy_pos.0} }
                ScheduledAction { execute_at: {now}, cast_duration: 0, actor: {player} }
                Pending
            });
            tl.phase = TurnPhase::Resolving;
            tl.reaction_resolved = true;
            log.push("[翻滚取消] 攻击中断，本回合转为翻滚".to_string());
            info!(
                "[翻滚取消] 攻击中断！消耗 {ROLL_CANCEL_COST} 精力（剩余 {}），本回合转为翻滚后退",
                stamina.current
            );
        }
        2 => {
            // 招架：消耗精力，声明绑定到敌人攻击实体的 Parry
            if !stamina.try_spend(PARRY_COST) {
                info!(
                    "[招架] 精力不足（需 {PARRY_COST}，当前 {}）—— 仍在反应阶段",
                    stamina.current
                );
                return; // 留在 Reaction，等待再次选择
            }
            let enemy_attack = pending_attacks
                .iter()
                .find(|(_, scheduled, _)| scheduled.actor == enemy)
                .map(|(entity, ..)| entity);
            if let Some(target_attack) = enemy_attack {
                let now = time.elapsed().as_millis() as u64;
                commands.spawn_scene(bsn! {
                    Parry { target_attack: {target_attack} }
                    ScheduledAction { execute_at: {now}, cast_duration: 0, actor: {player} }
                    Pending
                });
            }
            tl.phase = TurnPhase::Resolving;
            tl.reaction_resolved = true;
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
