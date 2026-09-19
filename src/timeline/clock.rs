//! 这个文件回答：**世界什么时候冻结**——原因集合（[`PauseReasons`]）+ 请求
//! （[`PauseRequest`]）+ 三个系统（断言空槽 / 每帧重建原因集合 / 唯一写时钟）。
//!
//! 冻结的判据只有一条：**本帧的暂停原因集合非空**。谁这一帧还想让世界停着，
//! 就断言一条原因；不再断言，原因下一帧自然消失——因此既不存在"原因留在集合里
//! 没人摘"的幽灵冻结，多种原因（等输入 / 手动 / 威胁）又能叠加、互不覆盖。
//!
//! 两条纪律里的第一条——**暂停只经 `PauseRequest`**：这一域里除了 [`apply_clock`]
//! 谁也不碰 `Time<Virtual>`，各领域因此不需要任何 `if paused` 分支。
//!
//! ```text
//! compute_player_awaiting_system（断言 "slot_empty"）
//!        └─▶ process_pause_requests（每帧重建原因集合）
//!                └─▶ apply_clock（唯一的 Time<Virtual> 写入点）
//! ```
//!
//! 后两个系统都排在帧末的 [`ClockSet`](super::ClockSet)：这一帧里所有系统看到的
//! 都是同一个时钟状态，不会出现"半帧冻、半帧不冻"。

use std::collections::HashSet;

use bevy::prelude::*;

use super::decision::{DecisionSlot, InputDriven};

/// 停表 / 解冻请求。
///
/// **断言式**：谁这一帧还想让世界停着，就写一条 [`PauseRequest::Pause`]。
/// 不需要谁去"撤销"自己的原因——下一帧不再断言，原因自然消失
/// （[`PauseReasons`] 每帧重建）。
/// 「等玩家决策」与「威胁逼近」因此可以叠加、互不覆盖，也不会出现
/// "原因留在集合里没人摘"的幽灵冻结。
///
/// 写：[`crate::input`]（手动暂停，哪个键由输入域自己定）、时间线的
/// [`compute_player_awaiting_system`]、
/// `combat::reaction::detect_threat_system`；
/// 消费：[`process_pause_requests`]，
/// 再由 [`apply_clock`] 落到 `Time<Virtual>`。
///
/// 原因只是**给人看的**（HUD 直接显示 `labels()`），所以是 `&'static str` 常量：
/// 加一个新原因不需要改任何枚举，只要一个常量加一个断言点，且每帧断言零分配。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseRequest {
    /// 这一帧仍然想停表，原因是 `reason`
    Pause(&'static str),
    /// 解冻：虚拟时间立刻流动，并清空已经收集到的原因
    Resume,
}

/// 暂停原因：手动暂停。
pub const MANUAL: &str = "manual";
/// 暂停原因：有一名 `InputDriven` 的行动者空着决策槽，正等玩家决策。
pub const SLOT_EMPTY: &str = "slot_empty";
/// 暂停原因：combat 检测到有威胁瞄准玩家（见 `combat::reaction`）。
pub const THREAT: &str = "threat";

/// **本帧**的暂停原因集合：整个游戏唯一的冻结判据（`frozen ⟺ 非空`）。
///
/// 每帧由 [`process_pause_requests`] 重建：
/// 先清空，再把这一帧收到的 [`Pause`](PauseRequest::Pause) 断言放进来
/// （收到 [`Resume`](PauseRequest::Resume) 则当场清空）。
///
/// 集合内容同时是给玩家看的答案——「现在是谁在停世界」（HUD 显示
/// [`labels`](Self::labels)），原因因此是常量而不是临时字符串。
#[derive(Resource, Debug, Default, Clone)]
pub struct PauseReasons(HashSet<&'static str>);

impl PauseReasons {
    /// 现在冻着吗。
    pub fn is_frozen(&self) -> bool {
        !self.0.is_empty()
    }

    /// 某个原因在不在（[`crate::input`] 用它判断手动暂停的开 / 关）。
    pub fn contains(&self, reason: &str) -> bool {
        self.0.contains(reason)
    }

    /// 断言一个原因（已经在里面就什么也不做）。
    pub fn insert(&mut self, reason: &'static str) {
        self.0.insert(reason);
    }

    /// 全部撤掉（每帧重建与 `Resume` 都走它）。
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// 排序后的原因列表（HUD 展示用；排序让「同一批原因」永远显示成同一句话）。
    pub fn labels(&self) -> Vec<&'static str> {
        let mut labels: Vec<&'static str> = self.0.iter().copied().collect();
        labels.sort_unstable();
        labels
    }
}

/// 有 `InputDriven`（玩家）空着决策槽吗 → 世界该停下来等他。
///
/// 这是「无回合」里唯一的时间门控需求：敌人不等玩家，玩家一空闲，世界就停。
/// 没有 `InputDriven` 单位时（单测、组装之前）一律当作「不等输入」，避免把世界冻住。
///
/// **每帧断言**：还等着就再说一次。不需要谁去"撤销"——下一帧玩家动了，
/// 这里不再断言，原因自然从集合里消失（见 [`PauseRequest`]）。
pub fn compute_player_awaiting_system(
    actors: Query<&DecisionSlot, With<InputDriven>>,
    mut pause: MessageWriter<PauseRequest>,
) {
    if actors.iter().any(|slot| slot.is_empty()) {
        pause.write(PauseRequest::Pause(SLOT_EMPTY));
    }
}

/// 暂停请求 → **本帧**的暂停原因集合。
///
/// 每帧重建：先清空，再按**发出顺序**处理这一帧的请求。
///
/// - [`PauseRequest::Pause`] 是**断言**："我还想让世界停着"——谁不停断言，
///   它的原因下一帧就不在了，不需要谁去撤销；
/// - [`PauseRequest::Resume`] 是**解冻**："清空此刻已经收集到的原因，让时间流动"。
///
/// 顺序因此有意义，而且是确定的：输入域（`Resume` 的来源）排在
/// [`TimelineSet`](super::TimelineSet) 之前，各领域的断言排在它之后——
/// 玩家手动解冻的那一帧，仍然成立的断言（比如"还等着你决策"）会照常加回来。
pub fn process_pause_requests(
    mut requests: MessageReader<PauseRequest>,
    mut reasons: ResMut<PauseReasons>,
) {
    reasons.clear();
    for request in requests.read() {
        match request {
            PauseRequest::Pause(reason) => reasons.insert(reason),
            PauseRequest::Resume => reasons.clear(),
        }
    }
}

/// **唯一**写 `Time<Virtual>` 的地方：原因集合非空就冻表，否则解冻。
///
/// 它排在帧末的 [`ClockSet`](super::ClockSet)，因此影响的是**下一帧**——
/// 这一帧里所有系统看到的都是同一个时钟状态，不会出现"半帧冻、半帧不冻"。
pub fn apply_clock(reasons: Res<PauseReasons>, mut time: ResMut<Time<Virtual>>) {
    if reasons.is_frozen() {
        if !time.is_paused() {
            time.pause();
            debug!("⏸ 世界冻结：{:?}", reasons.labels());
        }
    } else if time.is_paused() {
        time.unpause();
        debug!("▶ 世界继续");
    }
}

/// 测试用的「手动暂停开关」：真实实现里这个闩住在 `input` 域
/// （空格切换它，然后每帧断言 [`MANUAL`]）。
#[cfg(test)]
#[derive(Resource, Default)]
pub(crate) struct ManualLatch(pub(crate) bool);

/// 把 [`ManualLatch`] 每帧翻译成一条暂停断言——真实实现里是
/// `input::keyboard::pause_input_system` 那一段。
#[cfg(test)]
pub(crate) fn assert_manual(latch: Res<ManualLatch>, mut pause: MessageWriter<PauseRequest>) {
    if latch.0 {
        pause.write(PauseRequest::Pause(MANUAL));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::test_support::timeline_app;

    #[test]
    fn frozen_is_exactly_the_reason_set_being_non_empty() {
        let mut reasons = PauseReasons::default();
        assert!(!reasons.is_frozen(), "没有原因就不冻结");

        reasons.insert(MANUAL);
        reasons.insert(SLOT_EMPTY);
        assert!(reasons.is_frozen());
        assert_eq!(
            reasons.labels(),
            vec![MANUAL, SLOT_EMPTY],
            "多个原因可以叠加，互不覆盖"
        );

        // 每帧重建走的就是 clear()：清空前一直冻着，清空后才解冻
        reasons.clear();
        assert!(!reasons.is_frozen(), "清空之后才解冻");
    }

    /// 断言式的暂停：只要还在断言，世界就一直冻着；不再断言，下一帧就解冻。
    ///
    /// 旧实现是边沿触发的（靠 `Local<Option<bool>>` 记上一次的状态），
    /// 一旦有别人清空原因集合，它就再也不会把原因加回来——世界在"还等着玩家
    /// 决策"的状态下悄悄跑起来。
    #[test]
    fn a_reason_lasts_exactly_as_long_as_it_keeps_being_asserted() {
        let mut app = timeline_app();
        let player = app
            .world_mut()
            .spawn((InputDriven, DecisionSlot::Empty))
            .id();

        app.update(); // 空槽 → 世界本来就冻着
        app.world_mut()
            .entity_mut(player)
            .insert(DecisionSlot::Windup);
        app.update();
        assert!(
            !app.world().resource::<Time<Virtual>>().is_paused(),
            "没有原因时世界应当流动"
        );

        app.world_mut().resource_mut::<ManualLatch>().0 = true;
        for _ in 0..6 {
            app.update();
            assert!(
                app.world().resource::<Time<Virtual>>().is_paused(),
                "只要还在断言，手动暂停就一直有效"
            );
        }

        app.world_mut().resource_mut::<ManualLatch>().0 = false;
        app.update();
        assert!(
            !app.world().resource::<Time<Virtual>>().is_paused(),
            "不再断言，原因下一帧就该消失"
        );
    }

    /// 空决策槽是另一个独立原因：它消失时手动暂停仍然有效。
    #[test]
    fn pause_reasons_do_not_override_each_other() {
        let mut app = timeline_app();
        let player = app
            .world_mut()
            .spawn((InputDriven, DecisionSlot::Empty))
            .id();

        app.update(); // slot_empty
        app.world_mut().resource_mut::<ManualLatch>().0 = true;
        app.update(); // manual + slot_empty
        assert_eq!(
            app.world().resource::<PauseReasons>().labels(),
            vec![MANUAL, SLOT_EMPTY],
            "两个原因可以同时挂着"
        );

        app.world_mut()
            .entity_mut(player)
            .insert(DecisionSlot::Windup);
        app.update();
        assert!(
            app.world().resource::<Time<Virtual>>().is_paused(),
            "槽不空了，但手动暂停还挂着"
        );
        assert_eq!(
            app.world().resource::<PauseReasons>().labels(),
            vec![MANUAL],
            "不再成立的原因应当自己消失，而不是等人来摘"
        );

        app.world_mut().resource_mut::<ManualLatch>().0 = false;
        app.update();
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    }

    /// `Resume` 的语义：清空**此刻已经收集到**的原因。
    ///
    /// 顺序因此是有意义的，而且是确定的——输入域（`Resume` 的来源）排在
    /// [`TimelineSet`] 之前，各领域的断言排在它之后。
    #[test]
    fn a_resume_only_clears_what_was_asserted_before_it() {
        let mut app = timeline_app();
        app.world_mut().resource_mut::<ManualLatch>().0 = true;

        // 解冻请求先到，同一帧里仍然成立的断言随后加回来
        app.world_mut().write_message(PauseRequest::Resume);
        app.update();
        assert!(
            app.world().resource::<Time<Virtual>>().is_paused(),
            "解冻只清掉此刻已收集的原因；同一帧里仍然成立的断言要照常加回来"
        );

        // 断言停掉之后，解冻才真的生效
        app.world_mut().resource_mut::<ManualLatch>().0 = false;
        app.world_mut().write_message(PauseRequest::Resume);
        app.update();
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    }
}
