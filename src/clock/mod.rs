//! # clock — 世界的冻结设施（通用，不认识任何领域）
//!
//! 回答一件事：**世界现在停不停**。判据只有一条——**本帧的暂停原因集合非空**，
//! 或者**玩家自己要停**（[`ManualPause`]）。
//!
//! **这一层不属于任何领域**：它不认识决策槽、不认识行动、不认识威胁，
//! 只认识"请求 → 集合 → 时钟"。谁有理由停表，谁就按自己的事实写一条
//! [`PauseRequest`]：
//!
//! | 谁 | 凭什么断言 | 原因常量 |
//! | :--- | :--- | :--- |
//! | [`crate::timeline`] | 认识决策槽（玩家还没决定） | [`AWAITING`] |
//! | [`crate::combat::reaction`] | 认识威胁 | [`THREAT`] |
//! | [`crate::input`] | 玩家的按键（**只发消息，不查状态**） | [`PauseRequest::Toggle`] |
//!
//! 因果是单向的：各领域依赖本层，本层**不依赖它们**。所以
//! `compute_player_awaiting_system`（认识决策槽的那 10 行）住在 `timeline` 里，
//! 不在这里。
//!
//! ```text
//! 各领域的断言（每帧）─┐
//! 玩家按键（一次）   ─┴─▶ process_pause_requests ─▶ Time<Virtual>
//!                          （唯一的时钟写入点）
//! ```
//!
//! 冻结的判据是**每帧重建**的：谁这一帧还想让世界停着就断言一条；不再断言，
//! 原因下一帧自然消失。因此既不存在"原因留在集合里没人摘"的幽灵冻结，
//! 多种原因（等输入 / 威胁）又能叠加、互不覆盖。
//!
//! 唯一的例外是玩家的手动暂停——它是**状态**不是**断言**（见 [`ManualPause`]）。

use std::collections::HashSet;

use bevy::prelude::*;

pub mod plugin;

pub use plugin::ClockPlugin;

/// 钟表系统集：每帧**最后**一段，唯一的 `Time<Virtual>` 写入点就在这里。
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClockSet;

/// 停表 / 解冻请求。
///
/// **两种时序，别混**：
///
/// | 变体 | 时间语义 | 谁写 |
/// | :--- | :--- | :--- |
/// | [`Pause`](Self::Pause) | **每帧断言**：我还想让世界停着。不再写，原因下一帧自己消失 | 各领域（等玩家决策 / 威胁逼近） |
/// | [`Toggle`](Self::Toggle) | **翻转冻结状态**：冻着就放开、没冻就停住 | 输入域（玩家按的键） |
///
/// `Toggle` **不带原因**：玩家自己按的那个「暂停」不需要在集合里留名字。
/// 入帧时读一次"现在冻着吗"、翻一下就是他要的；集合每帧重建，所以也不必
/// 另存一个"手动暂停开着吗"的闩（旧实现的 `LatchedReasons` 就是为它而生的，已删）。
///
/// 分开的原因：断言式原因**每帧都要重新声明**，而玩家按键是**一次性事件**。
/// 两者如果共用一条消息，就必须先猜"上一帧有没有人断言过这个原因"——而集合里
/// 同时躺着别人的原因，猜不准（曾经因此把"恢复"误判成"暂停"）。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseRequest {
    /// 这一帧仍然想停表，原因是 `reason`
    Pause(&'static str),
    /// 翻转世界的冻结状态：冻着就放开，没冻就停住。
    ///
    /// **放开不等于一切归零**：同一帧里仍然成立的断言会照常加回来（等玩家决策
    /// `awaiting`），所以"放开世界"不等于"跳过决策"。玩家的手动暂停**不留痕**——
    /// 他要的是"动起来"。
    Toggle,
}

/// 暂停原因：有一名 `InputDriven` 的行动者**还没决定**（`!slot.ready()`），正等他。
pub const AWAITING: &str = "awaiting";
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

/// 玩家的手动暂停：**一个布尔**，不是集合。
///
/// **为什么必须存在**：世界的规则是「没有原因就自动恢复」——玩家一声明，
/// `awaiting` 消失，世界必须动起来。所以"玩家要求停着"这条信息得有地方记，
/// 否则下一帧就被自动恢复吃掉（旧实现用 `LatchedReasons` 这个**集合**装它，
/// 那是用一个集合装一个布尔，冗余在这里）。
///
/// 它**不进 [`PauseReasons`]**：那个集合只回答「**别人**为什么在停表」
/// （HUD 展示用），玩家的手动暂停不是"别人"。
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ManualPause(pub bool);

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

/// **暂停请求 → 本帧的冻结状态**：唯一处理它们的地方，也是**唯一的时钟写入点**
/// （`apply_clock` 已并入这里——那两个系统本来就是同一件事被拆成两处）。
///
/// 三步：
///
/// 1. **收断言** → [`PauseReasons`]（这一帧谁想停表）；集合只装别人的原因；
/// 2. **翻转手动暂停** → [`ManualPause`]。判据是**入帧时的冻结状态**
///    （这一次是不是"玩家按了暂停"），`Toggle` 因此不需要带原因。
///    翻成"继续"时**当场清空集合**——玩家要的是动起来，不能被他刚按掉的原因再按回来；
/// 3. **落到时钟**：`PauseReasons 非空 || ManualPause` → 停，否则放开。
///
/// 清空集合同时是**威胁窗口的关窗信号**：威胁检测（`CombatSet`，本系统之后）
/// 下一帧读到"集合里没有 `THREAT`"，就关窗并记 `dismissed`——它不必知道
/// "玩家是不是按了空格"。
pub fn process_pause_requests(
    mut requests: MessageReader<PauseRequest>,
    mut reasons: ResMut<PauseReasons>,
    mut manual: ResMut<ManualPause>,
    mut time: ResMut<Time<Virtual>>,
) {
    // 先收齐：`MessageReader` 的游标只能前进，读两遍拿不到第二遍
    let requests: Vec<PauseRequest> = requests.read().copied().collect();
    // **入帧时**的冻结状态：这一帧所有判断都以它为准（Toggle 的语义、理由都在这里）
    let was_frozen = time.is_paused();

    // ① 这一帧的断言
    reasons.clear();
    for request in &requests {
        if let PauseRequest::Pause(reason) = request {
            reasons.insert(reason);
        }
    }

    // ② 翻转**世界的冻结状态**（判据是入帧时冻着吗，不是"手动开关开着吗"）：
    //    冻着 → 玩家要放开：清空集合 + 关掉手动开关（他按的是"继续"）；
    //    没冻 → 玩家要停住：打开手动开关。
    //
    //    判据必须看**时钟**而不是 `manual`：世界可能是被**别人的原因**冻着的
    //    （威胁），这时按空格要的是放开，而不是再叠一层暂停。
    if requests
        .iter()
        .any(|request| matches!(request, PauseRequest::Toggle))
    {
        if was_frozen {
            reasons.clear();
            manual.0 = false;
        } else {
            manual.0 = true;
        }
    }

    // ③ 落到时钟：两个来源取或。**唯一**写 `Time<Virtual>` 的地方
    let paused = reasons.is_frozen() || manual.0;
    if paused != time.is_paused() {
        if paused {
            time.pause();
            debug!(
                "⏸ 世界冻结：{:?}{}",
                reasons.labels(),
                if manual.0 { " + manual" } else { "" }
            );
        } else {
            time.unpause();
            debug!("▶ 世界继续");
        }
    }
}

/// 测试用的「玩家按了一下空格」：真实实现里是 `input::keyboard::pause_input_system`。
///
/// 它**只发一条 `Toggle`**，不查任何状态——与真实输入域同形。
#[cfg(test)]
#[derive(Resource, Default)]
pub(crate) struct ManualLatch(pub(crate) bool);

/// 把 [`ManualLatch`] 这一次的翻转翻成一条 `Toggle`（真实实现里由按键触发）。
#[cfg(test)]
pub(crate) fn assert_manual(latch: Res<ManualLatch>, mut pause: MessageWriter<PauseRequest>) {
    if latch.0 {
        pause.write(PauseRequest::Toggle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    /// 本域自己的小夹具：只装时钟设施。
    ///
    /// **刻意不依赖 timeline**——时钟层是通用的，它的测试不该为了造"某个领域
    /// 在断言"而拖上决策槽。用一个玩具领域原因（[`AWAITING`]）代表"有人要停表"。
    fn clock_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .init_resource::<PauseReasons>()
            .init_resource::<ManualPause>()
            .init_resource::<ManualLatch>()
            .add_message::<PauseRequest>()
            .add_systems(
                Update,
                (
                    assert_manual,
                    // 每帧映一次时钟：真实流水线里这一步在 ClockSet
                    process_pause_requests,
                )
                    .chain(),
            );
        app
    }

    /// 冒充"某个领域每帧断言"：还想要就一直说。
    fn assert_awaiting(mut pause: MessageWriter<PauseRequest>) {
        pause.write(PauseRequest::Pause(AWAITING));
    }

    #[test]
    fn frozen_is_exactly_the_reason_set_being_non_empty() {
        let mut reasons = PauseReasons::default();
        assert!(!reasons.is_frozen(), "没有原因就不冻结");

        reasons.insert(AWAITING);
        reasons.insert(THREAT);
        assert!(reasons.is_frozen());
        assert_eq!(
            reasons.labels(),
            vec![AWAITING, THREAT],
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
        let mut app = clock_app();
        // 冒充一个领域：它这时**还想要**停表
        app.add_systems(Update, assert_awaiting);

        for _ in 0..3 {
            app.update();
            assert!(
                app.world().resource::<PauseReasons>().contains(AWAITING),
                "还在断言 → 原因一直在"
            );
            assert!(app.world().resource::<Time<Virtual>>().is_paused());
        }

        // 领域不再断言（真实情形：玩家决定了，`awaiting` 就不写了）
        let mut app2 = clock_app();
        app2.update();
        assert!(
            !app2.world().resource::<Time<Virtual>>().is_paused(),
            "没有原因时世界应当流动"
        );
    }

    /// 手动暂停（`ManualPause`）和领域的原因**互不覆盖**：原因消失时玩家要的暂停仍在。
    ///
    /// 这正是两类东西必须分开的实证：领域的原因是**临时请求**（不重发就没了），
    /// 玩家的暂停是**状态**（保持到再按一次）。
    #[test]
    fn the_manual_pause_survives_a_domain_reason_disappearing() {
        /// 冒充一个领域：`true` 表示"我这一帧还想停表"。
        #[derive(Resource, Default)]
        struct DomainWants(bool);
        fn assert_when_wanted(wants: Res<DomainWants>, mut pause: MessageWriter<PauseRequest>) {
            if wants.0 {
                pause.write(PauseRequest::Pause(AWAITING));
            }
        }

        let mut app = clock_app();
        app.init_resource::<DomainWants>()
            .add_systems(Update, assert_when_wanted);

        // 世界在跑（没人要求停表）→ 玩家按空格停住
        app.update();
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
        app.world_mut().resource_mut::<ManualLatch>().0 = true;
        app.update();
        assert!(app.world().resource::<Time<Virtual>>().is_paused());
        assert!(
            app.world().resource::<PauseReasons>().labels().is_empty(),
            "手动暂停不写原因"
        );

        // 领域也开始要求停表：两个来源并存。
        // （先关掉测试的 Toggle 闩——不然它每帧都翻一次，而"刚放开"的那一帧
        // 按设计会吞掉同帧的断言。）
        app.world_mut().resource_mut::<ManualLatch>().0 = false;
        app.world_mut().resource_mut::<DomainWants>().0 = true;
        app.update();
        assert_eq!(
            app.world().resource::<PauseReasons>().labels(),
            vec![AWAITING],
            "领域的原因照常进集合"
        );
        assert!(app.world().resource::<Time<Virtual>>().is_paused());

        // 领域不再要求了——**玩家要的暂停还在**（不会被领域原因的消失带走）
        app.world_mut().resource_mut::<DomainWants>().0 = false;
        app.update();
        assert!(
            app.world().resource::<PauseReasons>().labels().is_empty(),
            "领域的原因应当自己消失，而不是等人来摘"
        );
        assert!(
            app.world().resource::<Time<Virtual>>().is_paused(),
            "玩家按的那次暂停不该被领域原因的消失带走"
        );
    }

    /// `Toggle` **不带原因**：手动暂停靠 `Time<Virtual>` 记着，集合里不出现它。
    ///
    /// 这是删掉 `LatchedReasons` 的根据——那个闩存在的唯一理由就是"记住玩家开过
    /// 手动暂停"，而时钟自己就是那个状态。
    #[test]
    fn a_toggle_pauses_the_world_without_writing_a_reason() {
        let mut app = clock_app();
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());

        app.world_mut().write_message(PauseRequest::Toggle);
        app.update();
        assert!(
            app.world().resource::<Time<Virtual>>().is_paused(),
            "翻一下就该停住"
        );
        assert!(
            app.world().resource::<PauseReasons>().labels().is_empty(),
            "而且**不写原因**——玩家的暂停不需要在集合里留名字"
        );

        // 不再发任何消息：世界一直冻着（状态在时钟上，不需要每帧重发）
        for _ in 0..3 {
            app.update();
            assert!(
                app.world().resource::<Time<Virtual>>().is_paused(),
                "手动暂停不该只生效一帧"
            );
        }
    }

    /// 冻着再翻 → 放开；而且**同一帧的断言不会把世界按回去**。
    ///
    /// 顺序（先 Toggle 后收断言）就是为了这一条：玩家按了继续，他要的是动起来。
    #[test]
    fn toggling_while_frozen_releases_the_world_and_swallows_that_frames_assertions() {
        let mut app = clock_app();
        app.world_mut().write_message(PauseRequest::Toggle);
        app.update();
        assert!(app.world().resource::<Time<Virtual>>().is_paused());

        // 同一帧里：既按了继续，又有人（比如威胁）在断言
        fn assert_threat(mut pause: MessageWriter<PauseRequest>) {
            pause.write(PauseRequest::Pause(THREAT));
        }
        app.add_systems(Update, assert_threat);
        app.world_mut().write_message(PauseRequest::Toggle);
        app.update();

        assert!(
            !app.world().resource::<Time<Virtual>>().is_paused(),
            "翻回继续 → 世界当场流动"
        );
        assert!(
            !app.world().resource::<PauseReasons>().contains(THREAT),
            "刚放开的那一帧不重建集合——否则玩家按了也走不掉"
        );

        // 下一帧没有被吞的理由：断言照常进集合
        app.update();
        assert!(
            app.world().resource::<PauseReasons>().contains(THREAT),
            "下一帧威胁该回来就回来（威胁检测也会在这时读到「集合空」而关窗）"
        );
    }

    /// 没有 `Toggle` 时，集合与时钟**对齐**：`PauseReasons 非空 ⟺ 冻着`。
    #[test]
    fn without_a_toggle_the_reason_set_matches_the_clock() {
        let mut app = clock_app();
        app.add_systems(Update, assert_awaiting);
        app.update();
        assert!(app.world().resource::<PauseReasons>().is_frozen());
        assert!(app.world().resource::<Time<Virtual>>().is_paused());

        // 换一个没人断言的 App：两边同时"不冻"
        let mut quiet = clock_app();
        quiet.update();
        assert!(!quiet.world().resource::<PauseReasons>().is_frozen());
        assert!(!quiet.world().resource::<Time<Virtual>>().is_paused());
    }
}
