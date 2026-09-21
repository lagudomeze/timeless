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
/// **两种时序，别混**：
///
/// | 变体 | 时间语义 | 谁写 |
/// | :--- | :--- | :--- |
/// | [`Pause`](Self::Pause) | **每帧断言**：我还想让世界停着。不再写，原因下一帧自己消失 | 各领域（等玩家决策 / 威胁逼近） |
/// | [`Toggle`](Self::Toggle) | **翻转冻结状态**：冻着就放开、没冻就停住 | 输入域（玩家按的键） |
///
/// 分开的原因：断言式原因**每帧都要重新声明**，而玩家按键是**一次性事件**。
/// 两者如果共用一条消息，就必须先猜"上一帧有没有人断言过这个原因"——而集合里
/// 同时躺着别人的原因，猜不准（曾经因此把"恢复"误判成"暂停"）。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseRequest {
    /// 这一帧仍然想停表，原因是 `reason`
    Pause(&'static str),
    /// 翻转世界的冻结状态（`reason` 只是**名义归属**，给 HUD 显示"谁停的表"）。
    ///
    /// - **冻着** → 清空全部原因、连手动开关一起丢掉，世界**立刻**恢复流动；
    /// - **没冻** → 停住世界，并把 `reason` 记进 [`LatchedReasons`]，
    ///   此后它每帧自己续上（所以手动暂停不会"只生效一帧"）。
    ///
    /// **放开不等于一切归零**：清空之后那一帧仍然成立的断言会照常加回来
    /// （等玩家决策 `slot_empty`），而"威胁"这类**边沿触发**的只惊动一次
    /// （见 `combat::reaction` 的 `Threatened`），所以按一下真的能走
    /// ——代价是那一击照常落地，**忍受伤害也是一种决策**。
    Toggle(&'static str),
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
/// 先清空，再把这一帧收到的 [`Pause`](PauseRequest::Pause) 断言放进来；
/// [`Toggle`](PauseRequest::Toggle) 则先翻**闩住的**（[`LatchedReasons`]）状态，
/// 翻开的那些每帧继续断言——所以手动暂停不会"只生效一帧"。
///
/// 集合内容同时是给玩家看的答案——「现在是谁在停世界」（HUD 显示
/// [`labels`](Self::labels)），原因因此是常量而不是临时字符串。
#[derive(Resource, Debug, Default, Clone)]
pub struct PauseReasons(HashSet<&'static str>);

/// **闩住的手动原因**：开关式暂停（手动 / 将来的调试开关）的持久状态。
///
/// 只存"玩家按下的开关"，不存断言式原因（`slot_empty` / `threat` 每帧自己说）。
/// 与 [`PauseReasons`] 分开是刻意的：后者是**这一帧谁在停表**（混着别人的原因），
/// 前者是**玩家自己按下的开关**。混用会让"按一下是暂停还是继续"猜错。
#[derive(Resource, Debug, Default, Clone)]
pub struct LatchedReasons(HashSet<&'static str>);

impl LatchedReasons {
    /// 开一个开关。
    pub fn insert(&mut self, reason: &'static str) {
        self.0.insert(reason);
    }

    /// 全部关掉（玩家按 `Toggle` 放开世界时走它——他要的是"动起来"）。
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// 还开着哪些开关。
    pub fn iter(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.0.iter().copied()
    }
}

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

    /// 全部撤掉（每帧重建走它）。
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

/// **翻转先落地（排在各领域断言之前）**：把这一帧的 `Toggle` 变成原因集合的实际变化。
///
/// 必须**早于**各领域的断言（`CombatSet`）：否则同一帧里"威胁还在断言
/// `Pause(THREAT)`"会把刚清掉的原因加回来——玩家按空格等于没按。
/// 早落地之后，威胁检测那一帧读到"集合里没有 `THREAT`"，就会关窗、不再断言。
///
/// 判据是"**此刻冻着吗**"（读的是上一帧的结论）：
///
/// - **冻着** → 清空原因、连手动开关一起丢掉（玩家要的是"动起来"）；
/// - **没冻** → 停住，并把 `reason` 记进 [`LatchedReasons`]，此后它每帧自己续上。
///
/// 只消费 `Toggle`：`Pause` 断言留给帧末的 [`process_pause_requests`]
/// （每个系统各有自己的 `MessageReader` 游标，因此互不抢消息）。
pub fn apply_pause_toggles_system(
    mut requests: MessageReader<PauseRequest>,
    mut reasons: ResMut<PauseReasons>,
    mut latched: ResMut<LatchedReasons>,
) {
    for request in requests.read() {
        let PauseRequest::Toggle(reason) = request else {
            continue;
        };
        if reasons.is_frozen() {
            reasons.clear();
            latched.clear();
        } else {
            latched.insert(reason);
        }
    }
}

/// 暂停请求 → **本帧**的暂停原因集合（帧末，`ClockSet` 里 `apply_clock` 之前）。
///
/// 重建 = 开着的**手动开关**（[`LatchedReasons`]，每帧自己续上）+ 这一帧各领域的
/// [`Pause`](PauseRequest::Pause) 断言。断言会照常加回来，这正是"放开不是免费的"：
/// 按一下能清掉威胁窗口，但"还等着你决策"（`slot_empty`）会立刻把世界按回去。
pub fn process_pause_requests(
    mut requests: MessageReader<PauseRequest>,
    mut reasons: ResMut<PauseReasons>,
    latched: Res<LatchedReasons>,
) {
    let asserted: Vec<&'static str> = requests
        .read()
        .filter_map(|request| match request {
            PauseRequest::Pause(reason) => Some(*reason),
            PauseRequest::Toggle(_) => None, // 已经在 `apply_pause_toggles_system` 落地
        })
        .collect();

    reasons.clear();
    for reason in latched.iter() {
        reasons.insert(reason);
    }
    for reason in asserted {
        reasons.insert(reason);
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

    /// `Toggle` 是**翻转状态**，不是每帧断言：翻开关一次就够，之后它自己续上。
    ///
    /// 这条守住「两种时序别混」——断言式（`Pause`）每帧都要重新声明，
    /// 而玩家按键是一次性事件；混在一起就必须先猜"上一帧谁断言过这个原因"。
    #[test]
    fn a_toggle_flips_a_latch_that_then_keeps_asserting_itself() {
        let mut app = timeline_app();
        assert!(!app.world().resource::<PauseReasons>().is_frozen());

        app.world_mut().write_message(PauseRequest::Toggle(MANUAL));
        app.update();
        assert!(
            app.world().resource::<PauseReasons>().contains(MANUAL),
            "翻一下就该冻住"
        );

        // 不再发任何消息：开关自己每帧续上
        for _ in 0..3 {
            app.update();
            assert!(
                app.world().resource::<Time<Virtual>>().is_paused(),
                "翻开的开关不需要每帧重发消息"
            );
        }

        // 再翻一下：关掉
        app.world_mut().write_message(PauseRequest::Toggle(MANUAL));
        app.update();
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    }

    /// `Toggle` **没冻的时候**停住世界，而且那条原因会自己续上（不需要每帧重发）。
    ///
    /// 这条守住「两种时序别混」：断言式（`Pause`）每帧都要重新声明，
    /// 而玩家按键是一次性事件。混在一起就得先猜"上一帧谁断言过这个原因"。
    #[test]
    fn toggling_while_running_pauses_the_world() {
        let mut app = timeline_app();
        assert!(!app.world().resource::<PauseReasons>().is_frozen());

        app.world_mut().write_message(PauseRequest::Toggle(MANUAL));
        app.update();
        assert_eq!(
            app.world().resource::<PauseReasons>().labels(),
            vec![MANUAL],
            "没冻的时候翻一下 → 停住"
        );

        // 之后不再发消息：闩住的开关自己续上
        for _ in 0..3 {
            app.update();
            assert!(
                app.world().resource::<Time<Virtual>>().is_paused(),
                "闩住的原因不需要每帧重发"
            );
        }
    }

    /// `Toggle` **冻着的时候**放开世界——这正是"忍受伤害也是一种决策"的落点。
    ///
    /// 两条断言一起钉住"放开不是免费的"：**每帧仍在断言**的原因清不掉、
    /// 下一帧自己回来；而**开关式**的原因不会回来（它不每帧重发），所以
    /// 世界真的能走。
    #[test]
    fn toggling_while_frozen_releases_the_world() {
        let mut app = timeline_app();

        // 只有手动开关在停表（没有别人每帧断言）
        app.world_mut().write_message(PauseRequest::Toggle(MANUAL));
        app.update();
        assert!(app.world().resource::<Time<Virtual>>().is_paused());

        // 冻着再翻一下 → 清空并放开
        app.world_mut().write_message(PauseRequest::Toggle(MANUAL));
        app.update();
        assert!(
            !app.world().resource::<PauseReasons>().is_frozen(),
            "放开之后不该还留着手动开关（玩家要的是动起来）"
        );
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());

        // 而**每帧断言**的别人的原因清不掉：它在同一帧就回来了
        fn assert_threat_every_frame(mut pause: MessageWriter<PauseRequest>) {
            pause.write(PauseRequest::Pause(THREAT));
        }
        app.add_systems(Update, assert_threat_every_frame);
        app.update();
        app.world_mut().write_message(PauseRequest::Toggle(MANUAL));
        app.update();
        assert_eq!(
            app.world().resource::<PauseReasons>().labels(),
            vec![THREAT],
            "每帧断言的原因放开不掉——所以威胁必须做成**边沿触发**"
        );
    }
}
