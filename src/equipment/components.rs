//! # equipment — 装备领域
//!
//! 回答一个问题：**一个单位身上挂着什么，那些东西怎么改变它的战斗数值。**
//!
//! 装备**不引入新的战斗机制**：它是「往单位身上挂组件 / 改数值」的**来源**。
//! 一件主手给的是动作节奏的偏移与伤害加成，一件身甲给的是护甲加成，
//! 一面盾给的是格挡率——命中管线、调度器、技能目录都只是照常读那些组件。
//!
//! ## 三个实体角色（见 `docs/equipment.md`）
//!
//! ```text
//! PC（单位根）
//!  ├─ 槽位实体 EquipmentSlot { slot }      ← ChildOf(pc)：跟着 PC 动（物理附着）
//!  │    └─ （装上时）物品实体 Item { kind } ← EquippedTo(slot)：逻辑归属
//!  └─ 加成组件（ArmorBonus / …）           ← 本域是**唯一**写入者
//! ```
//!
//! 槽位挂 `ChildOf`、物品挂 `EquippedTo` 是刻意的分工：前者是"PC 的物理延伸"，
//! 后者是"装在哪个槽"（卸下时物品要活着）。完整推导见
//! [`relations.md`](../../docs/relations.md)。
//!
//! ## 属性叠加：基础值 + 加成
//!
//! **基础值由组装层写、装备不动它；加成只有本域写、且随时可以整个重算。**
//! 于是"没穿装备时我是什么样"永远查得到，而"卸下该减多少"这个记账问题
//! 根本不存在——重算是幂等的：把加成清零再加一遍，结果相同。
//!
//! 读取侧统一走纯函数（[`effective_armor`](super::effective_armor) /
//! [`effective_block_chance`](super::effective_block_chance)），公式只读它需要的分量。
//! 代价是读取点要多一次加法，换来的是基础值不会被装备永久覆盖。

use bevy::prelude::*;

/// 装备槽位的种类。
///
/// 这一版只做**主手 / 副手 / 身甲**三格。`Trinket`（饰品：改 Focus 上限 /
/// 回复速率）**故意不做**——它要改的是资源线而不是战斗数值，属于另一条玩法线；
/// 触发条件：出现"改资源"的装备需求，或 Focus 从时间线搬进 combat（`TODO.md` 的 T4）。
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SlotKind {
    /// 主手：动作节奏 + 伤害（决定"你是什么打法"）
    #[default]
    MainHand,
    /// 副手：格挡率 + 护甲（盾 / 副武器）
    OffHand,
    /// 身甲：护甲 + 移速修正
    Armor,
}

impl SlotKind {
    /// 全部槽位（组装层按它给每个 PC 建一个槽位实体）。
    pub const ALL: [Self; 3] = [Self::MainHand, Self::OffHand, Self::Armor];

    /// HUD / 日志用的英文短名。
    pub fn label(self) -> &'static str {
        match self {
            Self::MainHand => "main",
            Self::OffHand => "off",
            Self::Armor => "armor",
        }
    }
}

/// 物品的种类：**它就是这件装备提供什么**。
///
/// 数值写在这里而不是 `.ron`：这一版只有三件"起始装备"，等出现"装备来源"
/// （掉落 / 商店）时再外置成 `config/items.ron`——那时需要的是随机生成，
/// 而不是现在这样人手一件。
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemKind {
    /// 铁剑（主手）：伤害 +1、前摇 −0.05s
    Sword,
    /// 圆盾（副手）：格挡率 0.35、护甲 +1
    Shield,
    /// 锁子甲（身甲）：护甲 +1
    Mail,
}

impl ItemKind {
    /// 这件物品装在哪个槽（**唯一的类型真相**，校验 Observer 靠它）。
    pub fn slot(self) -> SlotKind {
        match self {
            Self::Sword => SlotKind::MainHand,
            Self::Shield => SlotKind::OffHand,
            Self::Mail => SlotKind::Armor,
        }
    }

    /// HUD / 日志用的英文短名。
    pub fn label(self) -> &'static str {
        match self {
            Self::Sword => "sword",
            Self::Shield => "shield",
            Self::Mail => "mail",
        }
    }

    /// 这个槽的**起始装备**（组装层用它给 PC 一身装备出生）。
    ///
    /// 它是"装备来源"这一层的替身：真正做掉落 / 商店时，`for_slot` 会消失，
    /// 物品由外部数据决定。现在它让"开局就能看到装备生效"成为可能。
    pub fn for_slot(slot: SlotKind) -> Self {
        match slot {
            SlotKind::MainHand => Self::Sword,
            SlotKind::OffHand => Self::Shield,
            SlotKind::Armor => Self::Mail,
        }
    }

    /// 它给单位的那点数值。
    pub fn bonus(self) -> ItemBonus {
        match self {
            // ⚠️ **占位数值**（与 `spawn::unit` 的护甲占位同性质）：选得刻意保守——
            // +1 伤害 / −0.05s 前摇都**不改变任何一击的刀数**，所以"装备接上了没有"
            // 这件事不会被平衡数值掩盖。格挡率是唯一例外：它从 0 变成 0.35，
            // 因为命中管线的第 ③ 关在此之前**从来没有被触发过**。
            Self::Sword => ItemBonus {
                damage: 1,
                windup_delta: -0.05,
                ..ItemBonus::default()
            },
            Self::Shield => ItemBonus {
                armor: 1,
                block_chance: 0.35,
                ..ItemBonus::default()
            },
            Self::Mail => ItemBonus {
                armor: 1,
                ..ItemBonus::default()
            },
        }
    }
}

/// 一件装备给单位的那点数值（**全是加成，不是覆盖**）。
///
/// 字段用**偏移**而不是绝对值，与第五节的"基础 + 加成"同形：技能的基础节奏
/// 住在 `config/actions.ron`，武器只说自己快多少——于是"配置里写的 0.30s、
/// 装上剑变成 0.25s"是一处加法，不需要第二套心智模型。
#[derive(Reflect, Debug, Default, Clone, Copy, PartialEq)]
pub struct ItemBonus {
    /// 护甲加成（加在 [`Armor`](crate::combat::Armor) 上）
    pub armor: i32,
    /// 伤害加成（加在载荷的原始 `PhysicalDamage` 上，见
    /// [`weapon_damage`](super::weapon_damage)）
    pub damage: i32,
    /// 前摇偏移（秒，负数 = 更快；**永不让前摇变成负数**）
    pub windup_delta: f32,
    /// 格挡率加成（加在
    /// [`BlockChance`](crate::combat::defense::BlockChance) 上）
    pub block_chance: f32,
}

/// 槽位实体：PC 的**物理延伸**（`ChildOf(pc)`）。
///
/// 它自己不是物品——只是"这个槽在这儿"。物品是独立实体（[`Item`]），
/// 卸下时断掉逻辑归属但仍然是活的。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[reflect(Component)]
pub struct EquipmentSlot {
    pub slot: SlotKind,
}

/// 物品实体：一件真的东西（有 `Transform`，能掉在地上 / 进背包）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct Item {
    pub kind: ItemKind,
}

impl Default for Item {
    /// 只为满足 BSN 模板约束（`Default + Clone`）；真实值一律由
    /// [`item_scene`](super::scene::item_scene) 给出。
    fn default() -> Self {
        Self {
            kind: ItemKind::Sword,
        }
    }
}

/// 一个单位身上**全部装备的加成之和**（本域是唯一写入者）。
///
/// 它是上面那份 [`ItemBonus`] 的聚合，而不是"每个属性一个组件"：
/// 读取侧（命中公式、攻击执行器、声明系统）都只要一份加成，没有一个消费者
/// 需要"单看护甲那一项加成"——所以拆成四个组件只会多三份写入者与三处同步。
/// 与设计稿的偏差记在 [`docs/equipment.md`](../../docs/equipment.md) 第五节。
///
/// **基础值不住在这里**：它就是单位身上原本那个组件（
/// [`Armor`](crate::combat::Armor) / [`BlockChance`](crate::combat::defense::BlockChance)），
/// 装备从不写它——于是"没穿装备时我是什么样"永远查得到。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Default)]
#[reflect(Component)]
pub struct EquipmentBonus(pub ItemBonus);

impl EquipmentBonus {
    /// 这一身装备给的护甲加成。
    pub fn armor(&self) -> i32 {
        self.0.armor
    }

    /// 这一身装备给的伤害加成。
    pub fn damage(&self) -> i32 {
        self.0.damage
    }

    /// 这一身装备给的前摇偏移（负数 = 更快）。
    pub fn windup_delta(&self) -> f32 {
        self.0.windup_delta
    }

    /// 这一身装备给的格挡率加成。
    pub fn block_chance(&self) -> f32 {
        self.0.block_chance
    }
}
