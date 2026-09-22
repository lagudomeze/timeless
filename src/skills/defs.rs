//! 一条技能定义长什么样。
//!
//! **这里只放数据**：不许出现 `Entity`、`Handle`、闭包或任何运行时状态——
//! 一旦出现，`AbilityDef` 就不再是可序列化的技能表，`.ron` 那条路当场断掉
//! （见 `docs/skills.md` 第一节）。

use crate::timeline::ActionTiming;

/// 技能的身份（注册表的键）。
///
/// 它是**静态目录的键**，不是调度表：没有人靠它去派发物化，
/// 谁声明谁物化（见本域模块文档）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbilityId {
    /// 走一格
    Move,
    /// 原地起跳
    Jump,
    /// 退一格 + 无敌帧
    Roll,
    /// 箭矢：朝一个方向射出去
    Shoot,
    /// 近战扇形横扫
    Melee,
    /// 锁格 AoE
    Fireball,
    /// 招架：挡下绑定的那次攻击并反制
    Parry,
    /// 原地等待：**占住决策槽一小段时间**，等价于"我要停一下"。
    ///
    /// 它是一条技能而不是特殊的暂停开关：无回合模型里"什么都不做"也是一种决定，
    /// 用既有的动作机制表达它（占槽 → `awaiting` 不再断言 → 世界继续跑），
    /// 因此不需要给暂停层加任何特例。玩家的空格直接绑定到它。
    Wait,
}

impl AbilityId {
    /// 全部技能（注册完整性用它自检）。
    pub const ALL: [Self; 8] = [
        Self::Move,
        Self::Jump,
        Self::Roll,
        Self::Shoot,
        Self::Melee,
        Self::Fireball,
        Self::Parry,
        Self::Wait,
    ];

    /// HUD / 日志用的英文短名。
    pub fn label(self) -> &'static str {
        match self {
            Self::Move => "move",
            Self::Jump => "jump",
            Self::Roll => "roll",
            Self::Shoot => "shoot",
            Self::Melee => "melee",
            Self::Fireball => "fireball",
            Self::Parry => "parry",
            Self::Wait => "wait",
        }
    }
}

/// 技能的类别。
///
/// 类别承担**共享条件**：`can_cast` 先按类别判一次（例如 Movement 一律要求没被定身），
/// 技能只写自己那几条 `requirements`——避免"每个技能把沉默 / 眩晕 / 冷却各写一遍"。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityCategory {
    /// 位移：移动 / 跳跃 / 翻滚
    Movement,
    /// 普通攻击：横扫 / 箭矢
    Attack,
    /// 法术：火球
    Spell,
    /// 姿态：招架这类不主动出手的动作
    Posture,
}

impl AbilityCategory {
    /// HUD / 日志用的英文短名。
    pub fn label(self) -> &'static str {
        match self {
            Self::Movement => "movement",
            Self::Attack => "attack",
            Self::Spell => "spell",
            Self::Posture => "posture",
        }
    }
}

/// 这一手需要什么目标（**设计意图**，不是几何：覆盖多大由 `Shape` 回答）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetSelector {
    /// 不需要目标（跳跃、招架自己找威胁）
    SelfOnly,
    /// 朝向一个方向（箭矢）
    Direction,
    /// 锁一格（火球落点、移动目标格）
    TargetCell,
    /// 打面前的扇形（横扫）
    MeleeArc,
    /// 指向某个实体（招架绑定的那次攻击）
    TargetEntity,
}

/// "这一手能不能被反制"——**设计事实**，写在定义上。
///
/// 标签做闸门、掷骰做对抗：`interruptible && !super_armor` 决定**能不能**打断，
/// `interrupt_lands` 决定**这一次**断不断（见 `docs/combat.md` 第二节）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CombatTags {
    /// 前摇中能不能被打断
    pub interruptible: bool,
    /// 霸体：豁免打断
    pub super_armor: bool,
    /// 能不能被招架
    pub parryable: bool,
    /// 能不能被格挡
    pub blockable: bool,
}

impl CombatTags {
    /// 普通攻击：能被打断、也能被招架 / 格挡。
    pub const STRIKE: Self = Self {
        interruptible: true,
        super_armor: false,
        parryable: true,
        blockable: true,
    };

    /// 起手就不再受打断影响（跳跃、翻滚、招架）。
    pub const COMMITTED: Self = Self {
        interruptible: false,
        super_armor: true,
        parryable: false,
        blockable: false,
    };
}

/// 拿这一手当**反制**要付什么代价（`None` = 这一手不能当反制）。
///
/// **它是技能的静态属性**，所以写在定义里：反制建议列表就是"遍历所有
/// `counter != None` 的技能"，没有硬编码的白名单（见 `docs/skills.md` 第四节）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterCost {
    /// 白送：反制插入，原决策保留
    Free,
    /// 花反制资源（当前是 [`crate::timeline::Focus`]），原决策保留
    Resource(u32),
    /// 拿原决策换：从时间轴移除原决策，插入反制
    CancelDecision,
}

/// 释放条件：**技能自己的那几条**（类别共享的那条另算，见 [`AbilityCategory::shared_requirement`]）。
///
/// ⚠️ **只列真的会被检查的条件**：现在只有精力。沉默 / 眩晕 / 冷却这些等它们
/// 真的存在了再加——预先堆一个用不上的枚举，只会让 `can_cast` 里长出一堆
/// 永远为真的分支（`docs/skills.md` 第三节的"现在不预先抽象"）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requirement {
    /// 精力够 `cost`
    EnoughEnergy,
}

/// 类别共享的条件：`can_cast` **先按类别判一次**，技能只写自己那几条。
///
/// 现在只有 Movement 一类有共享条件（有精力才能动），所以它是个返回
/// `Option` 的小函数而不是一张表——等出现第二类共享条件时再变成表。
impl AbilityCategory {
    /// 这一类**所有**技能都要满足的条件。
    pub fn shared_requirement(self) -> Option<Requirement> {
        match self {
            Self::Movement => Some(Requirement::EnoughEnergy),
            // 攻击 / 法术 / 姿态的消耗由技能自己的 `requirements` 说
            Self::Attack | Self::Spell | Self::Posture => None,
        }
    }
}

/// 一条技能定义：**静态、可序列化**。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbilityDef {
    pub id: AbilityId,
    pub category: AbilityCategory,
    /// 前摇 / 后摇 / 打断抗性（数值仍由各域给出，这里只聚合）
    pub timing: ActionTiming,
    pub targeting: TargetSelector,
    /// 精力消耗
    pub cost: u32,
    /// 释放条件（类别共享条件之外的）
    pub requirements: &'static [Requirement],
    /// 能不能被反制，以及当反制要付什么（见 `docs/skills.md` 第四节）
    pub combat: CombatTags,
    /// 能不能**当反制**用、当反制要付什么（`None` = 不能）
    pub counter: Option<CounterCost>,
    /// 大致威力（**展示用**：实际伤害在各自的载荷里）
    pub power: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ability_is_listed_once() {
        for id in AbilityId::ALL {
            assert_eq!(
                AbilityId::ALL.iter().filter(|other| **other == id).count(),
                1,
                "{} 在 ALL 里出现了不止一次",
                id.label()
            );
        }
    }

    /// 标签的两套预设就是设计意图：普通攻击能被反制，起手之后不能。
    #[test]
    fn the_two_tag_presets_say_what_they_mean() {
        let presets = [CombatTags::STRIKE, CombatTags::COMMITTED];
        let (strike, committed) = (presets[0], presets[1]);
        assert!(strike.interruptible && strike.parryable && strike.blockable);
        assert!(!strike.super_armor, "普通攻击不豁免打断");
        assert!(!committed.interruptible, "起手之后不再受打断影响");
        assert!(committed.super_armor);
    }
}
