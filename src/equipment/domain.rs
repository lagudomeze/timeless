//! 装备的**纯逻辑**（零 Bevy 依赖，可脱离 App 单测）。
//!
//! 只有三件事：有效数值怎么算、前摇偏移怎么夹、武器的伤害加成怎么落到一发攻击上。
//! 骰子 / 组件 / 实体一律不在这一层——那三样都属于应用层。

use super::components::ItemBonus;

/// 有效护甲 = 基础 + 加成。
///
/// "基础 + 加成"这个形状是刻意的：基础值由组装层写、装备不动它，
/// 于是"没穿装备时我是什么样"永远查得到（见本域模块文档）。
pub fn effective_armor(base: i32, bonus: i32) -> i32 {
    base + bonus
}

/// 有效格挡率 = 基础 + 加成，**夹在 `0..=1`**。
///
/// 夹在这里而不是让调用方自己夹：设计数据写错（三面盾？）不该让格挡率
/// 变成 1.5——那会让 `resolve_block` 的语义悄悄变味。
pub fn effective_block_chance(base: f32, bonus: f32) -> f32 {
    (base + bonus).clamp(0.0, 1.0)
}

/// 有效前摇 = 基础节奏 + 武器偏移，**永不为负**。
///
/// 为什么必须有这条下限：前摇是"声明到落地"的时长，负数会让
/// `execute_at` 落在声明之前——那一手会**倒退到过去**，而 `due(now)` 立刻为真，
/// 表现为"按下去就出手、谁也来不及反应"。这是整机级 bug，不是数值微调。
pub fn effective_windup(base: f32, delta: f32) -> f32 {
    (base + delta).max(0.0)
}

/// 一把武器让这一发打多少：载荷上的原始伤害 + 武器加成。
///
/// 伤害加成**加在原始伤害上、在护甲之前**（命中管线第 ④ 关之前）——
/// 于是"剑 +1"与"护甲 1"是两个独立的减法，顺序不影响结果，但读起来是
/// "剑更重"而不是"护甲更薄"。
pub fn weapon_damage(base: i32, bonus: i32) -> i32 {
    (base + bonus).max(0)
}

/// 把一堆物品的加成合成一份。
///
/// 加成的重算是**幂等**的：清空再加一遍结果相同——这正是"基础 + 加成"
/// 相对"写回原组件"的核心好处（不需要记住上次加了多少）。
pub fn sum_bonuses(bonuses: impl IntoIterator<Item = ItemBonus>) -> ItemBonus {
    bonuses
        .into_iter()
        .fold(ItemBonus::default(), |acc, item| ItemBonus {
            armor: acc.armor + item.armor,
            damage: acc.damage + item.damage,
            windup_delta: acc.windup_delta + item.windup_delta,
            block_chance: acc.block_chance + item.block_chance,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_values_are_base_plus_bonus() {
        assert_eq!(effective_armor(1, 2), 3);
        assert_eq!(effective_armor(0, 0), 0, "没装备时就是基础值");
        assert_eq!(effective_block_chance(0.0, 0.35), 0.35);
    }

    /// 格挡率被夹在 `0..=1`：设计数据写错不该改变 `resolve_block` 的语义。
    #[test]
    fn block_chance_is_clamped() {
        assert_eq!(effective_block_chance(0.8, 0.5), 1.0, "超过 1 夹到 1");
        assert_eq!(effective_block_chance(0.0, -0.5), 0.0, "负数夹到 0");
    }

    /// **前摇永不为负**：负数前摇会让 `execute_at` 落在声明之前（整机级 bug）。
    #[test]
    fn the_windup_never_goes_negative() {
        assert_eq!(effective_windup(0.30, -0.05), 0.25);
        assert_eq!(
            effective_windup(0.02, -1.0),
            0.0,
            "武器再快也不能让前摇倒退到过去"
        );
    }

    #[test]
    fn weapon_damage_adds_before_armor_and_never_goes_negative() {
        assert_eq!(weapon_damage(15, 1), 16);
        assert_eq!(weapon_damage(15, 0), 15, "没武器就是裸数值");
        assert_eq!(weapon_damage(0, -5), 0, "负伤害不产生治疗");
    }

    /// 多件装备的加成直接相加；重算是幂等的。
    #[test]
    fn bonuses_add_up_and_recomputing_is_idempotent() {
        let shield = ItemBonus {
            armor: 1,
            block_chance: 0.35,
            ..ItemBonus::default()
        };
        let mail = ItemBonus {
            armor: 1,
            ..ItemBonus::default()
        };

        let once = sum_bonuses([shield, mail]);
        assert_eq!(once.armor, 2);
        assert_eq!(once.block_chance, 0.35);
        // 幂等：再算一遍还是同一个数（"基础 + 加成"不需要记账）
        assert_eq!(sum_bonuses([shield, mail]), once);

        assert_eq!(
            sum_bonuses([]),
            ItemBonus::default(),
            "一件都不穿 = 全零，于是有效值就是基础值"
        );
    }
}
