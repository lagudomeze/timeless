//! 这个文件回答两个问题：**谁能决策**（槽 + 入口），以及**这一手怎么被撤掉 / 收尾**
//! （[`undo_system`] / [`recovery_system`]）。
//!
//! ## 谁能决策
//!
//! 行动者的**决策槽**：谁能声明行动，由它一个人说了算。
//!
//! 槽里装的是**决策**，不是**阶段**——「这一手执行到哪一步」由行动实体自己的
//! `ScheduledAction.execute_at` 回答（`now < execute_at` 前摇 / 过了该执行），
//! 槽不重复表达一遍：
//!
//! | 状态 | 含义 | 行动实体 |
//! | :--- | :--- | :--- |
//! | `Idle { intent: None }` | 空闲，可以声明 | 无 |
//! | `Idle { intent: Some(..) }` | 已决定、但这一手还没排进时间轴 | 无 |
//! | `Executing { until }` | 这一手在时间轴上，忙到 `until` | 有（前摇中）/ 已销毁（后摇中） |
//!
//! 判据只有一个（[`DecisionSlot::ready`]）：**这个单位此刻算不算「已经决定了」**。
//! 暂停断言（等玩家决策）只看它。
//!
//! 转换只有四条路，每条都只有一个作者：
//!
//! ```text
//! Idle ──声明（9 个声明系统）──▶ Executing { until } ──recovery_system──▶ Idle
//!   ▲                                    │
//!   └────── 撤销（undo_system）/ 打断（combat）──────┘
//! ```
//!
//! 「谁在写槽」因此是穷举的、可审计的；不会出现「标记忘了摘」这类
//! 状态与时间戳打架的 bug。
//!
//! **声明的入口只有 [`FirstReady::first_ready`] 一个**：「槽必须是空闲且没意图」
//! 这条判据与「被拒时告诉 HUD 为什么」都写在那里，各领域不再各抄一份。
//!
//! 决策来自玩家输入的单位由 [`InputDriven`] 标记：「这行动是谁的」由 [`ActionOf`] 回答，
//! 「这个单位听玩家的」则由这个标记回答。
//!
//! ## 这一手怎么被撤掉 / 收尾
//!
//! **撤销有两个来源**，它们汇合在同一个 [`undo_system`] 上：
//!
//! 1. [`UndoCommand`](super::events::UndoCommand)：玩家右键（`interaction`），
//!    将来可能的 `Esc`；
//! 2. [`PlayerTakeover`](super::events::PlayerTakeover)：玩家这一帧自己动手了
//!    （方向键 / 技能键 / 左键点击）。「随时可以改主意」就靠它——它曾经由一个
//!    名叫 `interrupt_system` 的系统翻译成 `UndoCommand`，现在直接和右键汇合，
//!    因为两者要做的判定完全一样。
//!
//! ⚠️ **只读输入层的事实**：`MoveCommand` / `RollCommand` / `UseSelectedSkill`
//! 这些是各领域的动作消息，时间线不该认识它们的词汇；而 `FireCommand` /
//! `MeleeCommand` 更是下游派生的，晚一帧才出现——监听它们会把"刚刚由自己的
//! 声明出来的行动"当成改主意撤掉（火球永远发不出去）。
//!
//! 两条纪律里的第二条——**收尾不集中**：执行器自己写 [`ScheduledAction`] 的判据、
//! 自己销毁行动实体、自己把行动者推进 `Recovery`——时间线只提供数据与判定函数，
//! 再加一个在 `until` 到点时清空槽的 [`recovery_system`]。

use bevy::prelude::*;

use super::events::{ActionBlocked, ActionCancelled, DecisionReady, PlayerTakeover, UndoCommand};
use super::ownership::ActionOf;
use super::schedule::{ActionTiming, ScheduledAction, Uncancellable};

/// 一次「已决定、还没排进时间轴」的决策。
///
/// ⚠️ **目前没有读者**：9 个声明系统都是"填意图 + **当场物化**"（各域自己物化，
/// 没有全局派发器），所以意图在同一帧里就被行动实体取代了。留着它是因为它是
/// 「决策」这个词的**形状**——`can_cast`（M24b）与将来的"先声明、后统一提交"
/// 都要读它。在等到第一个读者之前，`first_ready` 只依赖它的 `Some` / `None`。
#[derive(Reflect, Debug, Clone, Copy, PartialEq)]
pub struct Intent {
    /// 这一手是什么（[`crate::skills::AbilityId`]：移动 / 跳跃 / 翻滚 / 横扫 / 火球…）
    pub ability: crate::skills::AbilityId,
    /// 打哪儿 / 走哪儿
    pub target: Target,
}

/// 意图的目标（**设计意图**，不是几何：覆盖多大由形状组件回答）。
#[derive(Reflect, Debug, Clone, Copy, PartialEq)]
pub enum Target {
    /// 不需要目标（跳跃、招架自己找威胁）
    None,
    /// 锁一格（火球落点、移动目标格）
    Cell(crate::movement::Cell),
    /// 单体目标（招架绑定的那次攻击）
    Entity(Entity),
}

/// 行动者的决策槽状态机。
///
/// 「知道了没有」与「在不在执行」由这一个字段回答；前摇 / 后摇**不在这里**——
/// 它们在行动实体的 `execute_at` 与槽里的 `until` 上（见本文件文首的表格）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq)]
#[reflect(Component)]
pub enum DecisionSlot {
    /// 空闲：`intent` 空 = 还没决定；有 = 已决定、但这一手还没排进时间轴
    Idle { intent: Option<Intent> },
    /// 这一手已经在时间轴上（前摇 → 执行 → 后摇），`until` = 忙到什么时候
    Executing {
        /// 重新可决策时刻（虚拟秒）
        until: f32,
    },
}

/// 「忙着，而且永远不会到点」——**只给测试用**的哨兵。
///
/// 造"槽被占着"的场景时不能用 `until: 0.0`：那是**已经忙完**了，
/// `recovery_system` 下一帧就会把槽清空。
#[cfg(test)]
pub(crate) const BUSY_SENTINEL: f32 = f32::INFINITY;

impl Default for DecisionSlot {
    /// 出生就是「空闲、还没决定」——无回合模型里开局谁都可以决策。
    fn default() -> Self {
        Self::Idle { intent: None }
    }
}

impl DecisionSlot {
    /// 这个单位**此刻算不算「已经决定了」**——暂停断言只看这一条。
    ///
    /// - `Executing` → 是（他正忙着一手，别等他）；
    /// - `Idle { intent: Some }` → 是（他决定了，只是还没排期）；
    /// - `Idle { intent: None }` → **否**（正等他决策，世界该停下来）。
    pub fn ready(&self) -> bool {
        match self {
            Self::Executing { .. } => true,
            Self::Idle { intent } => intent.is_some(),
        }
    }

    /// 现在能不能声明行动（= 空闲**且**还没有意图）。
    ///
    /// 这是声明系统的入口判据；与 [`Self::ready`] 不同：`ready` 问"要不要等他"，
    /// 它问"能不能占这个槽"。
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle { intent: None })
    }

    /// 声明：**填意图 + 把这一手排进时间轴**。
    ///
    /// 无回合模型里没有「提交」这一步，所以声明即排期：`until` 由这一手的
    /// 前摇 + 后摇（[`ActionTiming::total`]）算出来。效果比后摇晚的动作
    /// （移动走到格中心、火球飞到落点）由**执行器收尾**时再往后推
    /// （见 [`Self::recovering`]），因为只有那时才知道要飞多久。
    ///
    /// `intent` 只用于**校验调用方确实想好了要做什么**，不进槽：行动在同一帧里
    /// 就被物化了（各域自己物化），槽立刻转成 `Executing`，`intent` 字段
    /// 没有机会被读到。等"先声明、后统一提交"落地时再把它存进去
    /// （那时 `Idle { intent: Some }` 这一段才有实际长度）。
    pub fn declared(_intent: Intent, timing: &ActionTiming, now: f32) -> Self {
        Self::Executing {
            until: now + timing.total(),
        }
    }

    /// 执行器收尾：进入后摇。
    ///
    /// 后摇从**效果真的发生**那一刻起算，因此带位移 / 飞行的动作要把
    /// 「效果还要多久才发生」传进来（移动走到格中心、火球飞到落点）；
    /// 瞬间完成的动作传 `0`，只忙一个后摇。
    ///
    /// ⚠️ **基准是 `schedule.execute_at`（这一手本该落地的时刻），不是"现在几点"**。
    /// 执行器是**轮询**的：它每帧问一次"到点了吗"，所以发现到点的那一帧总比
    /// `execute_at` 晚 0~1 个帧间隔。拿那一帧的 `now` 当基准，每个动作的忙碌窗口
    /// 都会偏长一个帧间隔（1s 的等待实测成 1.1s）。
    ///
    /// 传「到点时刻 + 时长」而不是让调用方算好「忙到哪个时刻」：
    /// 忙到的那一刻永远是 `execute_at + 这段时长`，基准统一在方法内部，
    /// 调用方不必各自记住该用哪个时间。
    pub fn recovering(
        timing: &ActionTiming,
        schedule: &ScheduledAction,
        effect_delay: f32,
    ) -> Self {
        Self::Executing {
            until: schedule.execute_at + timing.recovery.max(effect_delay),
        }
    }
}

/// 「这个查询项里带着行动者的决策槽」。
///
/// 为什么需要它：`QueryData` 的 item 是**元组**，Rust 没法从泛型元组里按类型取出
/// 某个分量。与其给 `Query<'w, 's, (…), F>` 写一堆带 GAT 的 impl，不如在**元组**
/// 上写几行——`Query::iter()` / `iter_mut()` 吐出来的就是元组本身。
///
/// 约定：**决策槽放在查询元组的最后一位**。这样只需要"每个元数一份"impl
/// （前面几个分量全是泛型，`&mut Stamina` / `Mut<Stamina>` 都能被吸收），
/// 不必为"槽在第几位"写组合数个版本。
pub trait HasDecisionSlot {
    /// 取出这一项里的决策槽。
    fn decision_slot(&self) -> &DecisionSlot;
}

impl<A> HasDecisionSlot for (A, &DecisionSlot) {
    fn decision_slot(&self) -> &DecisionSlot {
        self.1
    }
}

impl<A, B> HasDecisionSlot for (A, B, &DecisionSlot) {
    fn decision_slot(&self) -> &DecisionSlot {
        self.2
    }
}

impl<A, B, C> HasDecisionSlot for (A, B, C, &DecisionSlot) {
    fn decision_slot(&self) -> &DecisionSlot {
        self.3
    }
}

impl<A, B, C, D> HasDecisionSlot for (A, B, C, D, &DecisionSlot) {
    fn decision_slot(&self) -> &DecisionSlot {
        self.4
    }
}

impl<A, B, C, D, E> HasDecisionSlot for (A, B, C, D, E, &DecisionSlot) {
    fn decision_slot(&self) -> &DecisionSlot {
        self.5
    }
}

impl<A, B, C, D, E, F> HasDecisionSlot for (A, B, C, D, E, F, &DecisionSlot) {
    fn decision_slot(&self) -> &DecisionSlot {
        self.6
    }
}

/// **占一个决策槽的唯一入口**：挑出那个现在能决策的行动者，挑不到就替 HUD
/// 记下原因（[`ActionBlocked::BUSY`]）。
///
/// 迭代器上的一个方法，所以调用点是主语在前的一句话：
///
/// ```text
/// let Some((player, cell, _)) = players.iter().first_ready(&mut blocked) else {
///     return;
/// };
/// ```
///
/// 哪个分量是决策槽由 [`HasDecisionSlot`] 回答（槽在末位），因此这里不需要
/// 调用方再给一个投影闭包。判据只有一份的好处是：以后要放宽
/// （比如"后摇里也允许排下一手"）或改提示（比如区分"前摇中"与"后摇中"），
/// 只改这一个方法。
pub trait FirstReady: Iterator + Sized {
    /// 第一个**空闲且还没意图**的项；一个都没有就报一条 `BUSY`。
    fn first_ready(self, blocked: &mut MessageWriter<ActionBlocked>) -> Option<Self::Item>
    where
        Self::Item: HasDecisionSlot,
    {
        let ready = self
            .into_iter()
            .find(|actor| actor.decision_slot().is_idle());
        if ready.is_none() {
            // 静默丢弃是最差的手感：告诉 HUD"现在还动不了"
            blocked.write(ActionBlocked::BUSY);
        }
        ready
    }
}

impl<I: Iterator> FirstReady for I {}

/// 「这个单位的决策来自玩家输入」。
///
/// 时间线（等谁决策、冻结世界）与反应系统（谁被威胁）都只认这个标记，
/// 不再到处 `find(|faction| faction == Faction::Player)`：
/// `Faction` 管**战斗目标过滤**，`InputDriven` 管**输入归属**，两者语义不同。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct InputDriven;

/// 有 `InputDriven`（玩家）**还没决定**吗 → 让世界停下来等他。
///
/// 判据是 [`DecisionSlot::ready`]（"决定了没有"），不是"槽空不空"：
/// 声明之后槽进 `Executing`，那时他已经决定了，世界不该再等他。
///
/// 这是「无回合」里唯一的时间门控需求：敌人不等玩家，玩家一空闲，世界就停。
/// 没有 `InputDriven` 单位时（单测、组装之前）一律当作「不等输入」，避免把世界冻住。
///
/// **每帧断言**：还等着就再说一次。不需要谁去"撤销"——下一帧玩家动了，
/// 这里不再断言，原因自然从集合里消失（见 [`PauseRequest`](crate::clock::PauseRequest)）。
///
/// 它住在**本域**而不是 [`crate::clock`]：只有时间线认识决策槽。
/// 时钟层是通用设施，谁有理由停表谁按自己的事实写一条断言。
pub fn compute_player_awaiting_system(
    actors: Query<&DecisionSlot, With<InputDriven>>,
    mut pause: MessageWriter<crate::clock::PauseRequest>,
) {
    if actors.iter().any(|slot| slot.is_idle()) {
        pause.write(crate::clock::PauseRequest::Pause(crate::clock::AWAITING));
    }
}

/// 撤销：把玩家那条**还没到点**的行动撤掉——触发 [`ActionCancelled`]、销毁
/// 行动实体、清空决策槽。
///
/// 撤销有两个来源，缺一不可：
///
/// 1. 显式请求 [`UndoCommand`]：鼠标右键（`interaction`）、将来可能的 `Esc`；
/// 2. **玩家自己动手了** [`PlayerTakeover`]：这是「随时可以改主意」的机制——
///    声明系统看到"槽被占着"只会回一句 [`ActionBlocked`]，而玩家的真实意思是
///    "我要改手"。两个来源都读**这一帧的消息**，因此改主意抢在声明系统之前
///    把槽腾出来（本系统排在 [`TimelineSet`](super::TimelineSet) 里声明系统之前）。
///
/// 判定是纯谓词，两条同时满足才触发：① 撤销来源出现了（见上）；② 玩家有一条未执行
/// 的行动（本系统自己会检查「还没到点 / 可取消 / 是玩家」）。
///
/// 「还没到点」= `now < execute_at`。到点的行动这一帧就落地，撤不掉；
/// 没有可撤的行动时什么也不做（右键空放不报错，也不提示）。
///
/// 只撤**玩家**的行动：AI 的行动挂在同一条时间线上，但"谁能按键撤销"只有 PC
/// （和"谁能按键决策"是同一条约定：认 [`InputDriven`]）。
///
/// 挂着 [`Uncancellable`] 的行动直接跳过（跳跃那种"起跳就谁都别想插队"）。
pub fn undo_system(
    mut commands: Commands,
    mut requests: MessageReader<UndoCommand>,
    mut takeovers: MessageReader<PlayerTakeover>,
    drivers: Query<(), With<InputDriven>>,
    actions: Query<(Entity, &ScheduledAction, &ActionOf), Without<Uncancellable>>,
    time: Res<Time<Virtual>>,
) {
    // 撤销有两个来源：玩家右键（UndoCommand），或玩家自己动手了（PlayerTakeover）
    let explicit = requests.read().last().is_some();
    let player_took_over = takeovers.read().last().is_some();
    if !explicit && !player_took_over {
        return;
    }
    let now = time.elapsed_secs();
    for (entity, schedule, action_of) in &actions {
        // 行动者 = 父实体：归属由关系回答，不必在调度数据里再抄一份
        let actor = action_of.actor();
        if !schedule.pending(now) {
            continue; // 本帧就要执行，来不及撤
        }
        if drivers.get(actor).is_err() {
            continue; // AI 的行动只能被「打断」，不能被右键撤
        }
        // 先触发再销毁：Observer 当场跑，排在 despawn 之后就读不到载荷了
        commands.trigger(ActionCancelled { entity, actor });
        commands.entity(entity).despawn();
        // 行动者可能已经死了：往不存在的实体上写命令会让 Bevy 直接 panic
        if let Ok(mut actor_commands) = commands.get_entity(actor) {
            actor_commands.insert(DecisionSlot::Idle { intent: None });
        }
        break; // 一次决策只有一条行动
    }
}

/// 忙完：`Executing { until }` 到点就清空决策槽，并广播 [`DecisionReady`]。
///
/// 「又轮到它决策了」这件事只由时间线宣布；因此**谁能拿到它**（比如精力回复）
/// 由关心它的领域自己写 observer——时间线不反向依赖任何资源。
pub fn recovery_system(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    mut recovering: Query<(Entity, &mut DecisionSlot)>,
) {
    let now = time.elapsed_secs();
    for (entity, mut slot) in &mut recovering {
        // 只处理"在时间轴上"的：空闲没什么可恢复的，意图也不用在这里清
        let DecisionSlot::Executing { until } = *slot else {
            continue;
        };
        if now < until {
            continue;
        }
        *slot = DecisionSlot::Idle { intent: None };
        commands.trigger(DecisionReady { entity });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::test_support::{Cancellations, timeline_app};

    /// 调度器的测试不该依赖任何具体载荷：自己造一个节奏。
    const TEST_TIMING: ActionTiming = ActionTiming::new(0.2, 0.3, 3);

    /// 出生就是「空闲、还没决定」——**注意这时 `ready()` 是 false**：
    /// `ready` 问的是"该不该等他"，而空闲的人正是要等的那一个。
    #[test]
    fn a_fresh_slot_is_idle_and_not_ready() {
        let fresh = DecisionSlot::default();
        assert_eq!(fresh, DecisionSlot::Idle { intent: None });
        assert!(fresh.is_idle(), "空闲且没意图 → 可以声明");
        assert!(!fresh.ready(), "还没决定 → 世界该等他");
    }

    /// 两个判据问的是不同的事，别混：
    ///
    /// - `is_idle`：**能不能占这个槽**（声明系统的入口）
    /// - `ready`：**要不要等他**（暂停断言）
    ///
    /// `Executing` 两边都是"占着 + 别等"；`Idle { Some }` 是"占着 + 别等"
    /// （他决定了，只是还没排期）——这时 `is_idle` 为假，所以不会有人挤掉他的意图。
    #[test]
    fn the_two_predicates_answer_different_questions() {
        let executing = DecisionSlot::Executing { until: 1.0 };
        assert!(!executing.is_idle(), "在时间轴上 → 不能声明");
        assert!(executing.ready(), "忙着一手 → 别等他");

        let decided = DecisionSlot::Idle {
            intent: Some(Intent {
                ability: crate::skills::AbilityId::Move,
                target: Target::None,
            }),
        };
        assert!(!decided.is_idle(), "已经有意图了 → 不能再声明");
        assert!(decided.ready(), "已经决定了 → 别等他");
    }

    /// 后摇取「一个后摇」与「效果还要多久」里更晚的那个，**基准是 `execute_at`**。
    #[test]
    fn the_recovery_window_ends_at_the_later_of_effect_and_recovery() {
        let due_at = |t: f32| ScheduledAction::immediate(t);

        // 效果比后摇晚（移动 / 火球）：忙到效果真的发生
        assert_eq!(
            DecisionSlot::recovering(&TEST_TIMING, &due_at(1.0), 0.4),
            DecisionSlot::Executing { until: 1.4 }
        );
        // 效果瞬间完成（近战 / 招架）：只忙一个后摇
        assert_eq!(
            DecisionSlot::recovering(&TEST_TIMING, &due_at(1.0), 0.0),
            DecisionSlot::Executing {
                until: 1.0 + TEST_TIMING.recovery
            }
        );
    }

    /// **基准是到点时刻，不是"发现到点的那一帧"**——忙碌窗口因此不随帧率漂移。
    ///
    /// 执行器是**轮询**的：它在 `execute_at` 之后的第一帧才发现到点，所以那一帧的
    /// `now` 总比 `execute_at` 晚一点。曾经拿它当基准，每个动作的忙碌窗口于是都偏长
    /// 0~1 个帧间隔（1s 的等待实测成 1.1s）。
    #[test]
    fn the_recovery_window_does_not_depend_on_when_the_executor_notices() {
        // 声明于 0.0，前摇 0.2 → 本该在 0.2 落地
        let declared = ScheduledAction::declared_at(TEST_TIMING, 0.0);
        let until = match DecisionSlot::recovering(&TEST_TIMING, &declared, 0.0) {
            DecisionSlot::Executing { until } => until,
            other => panic!("收尾之后应当在执行中，实际 {other:?}"),
        };

        assert_eq!(
            until,
            declared.execute_at + TEST_TIMING.recovery,
            "窗口 = 到点时刻 + 后摇，与执行器第几帧才发现无关"
        );
    }

    /// 挑人：空槽的中选；一个都没有就替 HUD 记一条 `BUSY`。
    ///
    /// 这条守着「声明的判据只有一份」——9 个声明系统原来各写一遍这段，
    /// 现在只有 `first_ready` 会写 `ActionBlocked`。
    #[test]
    fn first_ready_picks_the_first_empty_slot_or_reports_busy() {
        #[derive(Resource, Default)]
        struct Picked {
            actor: Option<Entity>,
            blocks: usize,
        }

        fn pick(
            actors: Query<(Entity, &DecisionSlot)>,
            mut blocked: MessageWriter<ActionBlocked>,
            mut out: ResMut<Picked>,
        ) {
            out.actor = actors
                .iter()
                .first_ready(&mut blocked)
                .map(|(entity, _)| entity);
        }

        fn count(mut requests: MessageReader<ActionBlocked>, mut out: ResMut<Picked>) {
            out.blocks += requests.read().count();
        }

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Picked>()
            .add_message::<ActionBlocked>()
            .add_systems(Update, (pick, count).chain());

        // 全是忙的：挑不到人，而且要报一条原因
        app.world_mut().spawn(DecisionSlot::Executing {
            until: BUSY_SENTINEL,
        });
        app.update();
        let picked = app.world().resource::<Picked>();
        assert_eq!(picked.actor, None, "没人空着就挑不到");
        assert_eq!(picked.blocks, 1, "被拒时要替 HUD 记一条原因");

        // 来了个空槽的：挑中他，而且不再报原因
        let ready = app
            .world_mut()
            .spawn(DecisionSlot::Idle { intent: None })
            .id();
        app.update();
        let picked = app.world().resource::<Picked>();
        assert_eq!(picked.actor, Some(ready), "空槽的人中选");
        assert_eq!(picked.blocks, 1, "挑到了就不该再报一条");
    }

    /// 后摇到点：`Recovery { until }` → `Empty`，而且**只**碰后摇。
    #[test]
    fn recovery_clears_the_slot_only_after_the_window_ends() {
        fn slot_of(app: &App, entity: Entity) -> Option<DecisionSlot> {
            app.world().get::<DecisionSlot>(entity).copied()
        }

        let mut app = timeline_app();
        let actor = app
            .world_mut()
            .spawn(DecisionSlot::Executing { until: 0.35 })
            .id();
        let winding_up = app
            .world_mut()
            .spawn(DecisionSlot::Executing {
                until: BUSY_SENTINEL,
            })
            .id();

        // 「到点」是唯一的清槽条件：虚拟时间没走到 until 就一直留着
        let mut frames = 0;
        while app.world().resource::<Time<Virtual>>().elapsed_secs() < 0.35 {
            assert_eq!(
                slot_of(&app, actor),
                Some(DecisionSlot::Executing { until: 0.35 }),
                "后摇没到点不该清槽"
            );
            app.update();
            frames += 1;
            assert!(frames < 20, "虚拟时间没有推进，后摇永远不会结束");
        }
        assert_eq!(
            slot_of(&app, actor),
            Some(DecisionSlot::Idle { intent: None }),
            "后摇到点应当清空决策槽"
        );
        assert_eq!(
            slot_of(&app, winding_up),
            Some(DecisionSlot::Executing {
                until: BUSY_SENTINEL
            }),
            "前摇归执行器与撤销 / 打断管，后摇系统不该碰它"
        );
    }

    /// 撤销：广播 [`ActionCancelled`]、销毁行动实体、清空决策槽。
    ///
    /// 退多少归花钱的领域（见 `combat::attack` 的退款 Observer），这里只保证
    /// **广播打在行动实体上、而且发生在销毁之前**——排在销毁之后的话，
    /// Observer 已经读不到载荷，退款会静默丢失。
    #[test]
    fn undo_broadcasts_the_cancellation_before_despawning() {
        let mut app = timeline_app();
        let player = app
            .world_mut()
            .spawn((
                InputDriven,
                DecisionSlot::Executing {
                    until: BUSY_SENTINEL,
                },
            ))
            .id();
        let action = app
            .world_mut()
            .spawn((
                ActionOf(player),
                ScheduledAction::declared_at(TEST_TIMING, 0.0),
            ))
            .id();

        app.world_mut().write_message(UndoCommand);
        app.update();

        assert_eq!(
            app.world().resource::<Cancellations>().0,
            vec![(action, player)],
            "撤销要广播在行动实体上，并带上行动者"
        );
        assert!(
            app.world().get_entity(action).is_err(),
            "撤销要把行动实体销毁"
        );
        assert_eq!(
            app.world().get::<DecisionSlot>(player).copied(),
            Some(DecisionSlot::Idle { intent: None }),
            "撤销之后应当立刻能重新决策"
        );
    }

    /// 挂着 [`Uncancellable`] 的行动：连右键也撤不掉（跳跃那种"起跳不插队"）。
    #[test]
    fn an_uncancellable_action_survives_the_undo() {
        let mut app = timeline_app();
        let player = app
            .world_mut()
            .spawn((
                InputDriven,
                DecisionSlot::Executing {
                    until: BUSY_SENTINEL,
                },
            ))
            .id();
        let action = app
            .world_mut()
            .spawn((
                ActionOf(player),
                ScheduledAction::declared_at(TEST_TIMING, 0.0),
                Uncancellable,
            ))
            .id();

        app.world_mut().write_message(UndoCommand);
        app.update();

        assert!(
            app.world().get_entity(action).is_ok(),
            "不可撤销的行动不该被右键打掉"
        );
        assert!(
            app.world().resource::<Cancellations>().0.is_empty(),
            "没撤掉就不该广播撤销"
        );
        assert_eq!(
            app.world().get::<DecisionSlot>(player).copied(),
            Some(DecisionSlot::Executing {
                until: BUSY_SENTINEL
            }),
            "行动还在，决策槽也还占着"
        );
    }

    /// 到点的行动撤不掉：`execute_at <= now` 就是"这一手已经出去了"。
    #[test]
    fn an_action_that_already_came_due_cannot_be_undone() {
        let mut app = timeline_app();
        let player = app
            .world_mut()
            .spawn((
                InputDriven,
                DecisionSlot::Executing {
                    until: BUSY_SENTINEL,
                },
            ))
            .id();
        let action = app
            .world_mut()
            .spawn((
                ActionOf(player),
                ScheduledAction::declared_at(TEST_TIMING, 0.0),
            ))
            .id();

        // 世界走了 1 秒：0.15s 的前摇早就过了
        app.world_mut().resource_mut::<Time<Virtual>>().unpause();
        for _ in 0..12 {
            app.update();
        }
        app.world_mut().write_message(UndoCommand);
        app.update();

        assert!(
            app.world().get_entity(action).is_ok(),
            "已经到点的行动不该被撤销"
        );
    }

    /// 撤销只认**输入层的事实**这一条消息：时间线不认识各领域的命令词汇。
    #[derive(Resource, Default)]
    struct Undos(usize);

    fn count_undos(_cancelled: On<ActionCancelled>, mut undos: ResMut<Undos>) {
        undos.0 += 1;
    }

    #[test]
    fn only_a_player_intent_asks_for_an_undo() {
        let mut app = timeline_app();
        app.init_resource::<Undos>().add_observer(count_undos);
        let player = app
            .world_mut()
            .spawn((
                InputDriven,
                DecisionSlot::Executing {
                    until: BUSY_SENTINEL,
                },
            ))
            .id();
        app.world_mut().spawn((
            ActionOf(player),
            ScheduledAction::declared_at(TEST_TIMING, 0.0),
        ));

        app.update();
        assert_eq!(app.world().resource::<Undos>().0, 0, "没人表态就不该撤销");

        app.world_mut().write_message(PlayerTakeover);
        app.update();
        assert_eq!(
            app.world().resource::<Undos>().0,
            1,
            "一句「玩家动手了」就够时间线撤掉那条还没到点的行动"
        );
    }
}
