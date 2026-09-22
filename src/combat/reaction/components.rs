//! 反应系统组件：威胁声明（行动侧）+ **已经惊动过玩家** 的记账。

use bevy::prelude::*;

use crate::movement::Cell;

/// 「这条行动威胁到哪些格」——**由行动自己声明**。
///
/// - 火球：飞过的格 + 落点（[`trajectory_cells`](super::trajectory_cells)）；
/// - 近战：正前方一格 + 左右各一格（[`melee_arc_cells`](super::melee_arc_cells)）；
/// - 移动 / 跳跃 / 撤退：不挂这个组件（走过去不构成威胁）。
///
/// 用「格」而不是「实体」：威胁问的是"我站的地方安不安全"，
/// 而决策层本来就是按格算的（见 docs/timeline.md 第二节）。
#[derive(Component, Debug, Default, Clone, PartialEq, Eq)]
pub struct Threatens {
    pub cells: Vec<Cell>,
}

/// 飞行中的投射物瞄准的格。
///
/// 投射物不是行动实体（它的"意愿"早就确定了），因此单独用这个组件声明威胁：
/// 一枚正在飞向玩家脚下格的火球，和一条还在前摇的火球行动一样危险。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TargetCell(pub Cell);

/// **这条威胁已经惊动过玩家了**——边沿触发的一次性记账。
///
/// 挂在**威胁源**（那条行动实体 / 那颗投射物）上。检测系统只对「还没有这个标记、
/// 且瞄着玩家」的来源开一次冻结；打上标记之后它就不再触发，于是：
///
/// - 玩家按空格（`PauseRequest::Toggle`）**真的能走**——那一击照常落地，代价自负；
/// - 同一个来源不会"你按一次它冻一次"，不需要玩家在窗口之外再表态。
///
/// **为什么记在威胁源上而不是全局**：全局记账（旧的 `ThreatWindow`）只能表达
/// "现在有没有威胁"，于是每帧都得重新判断"玩家表态了没有"——而那个判断依赖
/// "玩家那一手变了没有"，在后摇 / 不可撤行动期间永远为假。记在源上之后，
/// 「惊动过」是源自己的属性，与玩家在哪个阶段无关。
///
/// 挂标记的时机**必须晚于** `detect_threat_system` 的读取（同帧写入会漏检），
/// 因此它由 `[`mark_threatened_system`](super::mark_threatened_system)` 在检测
/// **之后**按本帧的检测结果打上；来源被销毁时标记随实体一起消失。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Threatened;

/// 一次威胁开的**反应窗口**——挂在**被威胁的玩家**身上。
///
/// 它是「玩家欠一个表态」这件事的唯一真相。与决策槽**共存、互不改结构**：
/// 反制是独立资源，不覆盖决策槽。
///
/// ```text
/// 威胁出现（取最先落地的那一个）→ 挂上 ReactionSlot + suggestions
///                                          ↓ 每帧断言 Pause(THREAT)
/// 玩家表态（技能键 / 右键放弃 / 空格）→ resolved = true → 不再断言
/// 威胁自己消失（被打断 / 落地 / 投射物没了）→ 槽移除
/// ```
///
/// **退出只有两条**（设计如此）：表态，或者威胁消失。玩家什么都不做时世界
/// 一直冻着——战术暂停里"我在想"必须能无限期地想下去。
#[derive(Component, Debug, Clone, PartialEq)]
pub struct ReactionSlot {
    /// 是哪条行动 / 哪颗投射物
    pub threat: Entity,
    /// 能拿哪几手反制（见 [`CounterSuggestion`]）
    pub suggestions: Vec<CounterSuggestion>,
    /// 玩家表态了没有
    pub resolved: bool,
}

/// 一条反制建议 = **一个技能 + 它作为反制要付的代价**。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CounterSuggestion {
    pub ability: crate::skills::AbilityId,
    /// 技能的静态属性（见 `docs/skills.md` 第四节）
    pub cost: crate::skills::CounterCost,
    /// 现在付得起吗（HUD 决定亮不亮）
    pub affordable: bool,
}
