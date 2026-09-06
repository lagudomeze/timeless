//! # 战斗机制：纯函数判定（纯 Rust，零引擎依赖）
//!
//! 迁移自旧 `timeless-domain::combat`，但**不再聚合 `AttackStats` 属性包**。
//!
//! 每个机制都是独立的纯函数，只收自己机制需要的标量：
//!
//! | 机制 | 组件载体（引擎侧） | 判定入口 |
//! | :--- | :--- | :--- |
//! | 射程 | `AttackRange` | [`within_range`] |
//! | 同刻先后（帧 → 射程） | `AttackFrame` / `AttackRange` | [`earlier_side`] |
//! | 破势对抗 | `Impact` | [`impact_breaks`] |
//! | 伤害数值 | `Damage` | 命中后由引擎侧伤害应用系统直接扣 `Health` |
//!
//! ECS 分工：组件负责“数据归属”，系统负责“读取自己机制的组件”，
//! 互不越界；本层只回答单个机制的问题，不产出 OO 式的整包裁决结果。

use std::cmp::Ordering;

/// 交锋中的一方（描述 `Option<Side>` 指向哪一方）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    First,
    Second,
}

/// 射程机制：`distance`（网格距离）是否在攻击射程内。
/// 单方攻击与双方攻击的各自命中判定都复用本入口。
pub fn within_range(attack_range: u32, distance: u32) -> bool {
    distance <= attack_range
}

/// 同刻先后机制：两个攻击在完全同一时刻命中彼此时，先比帧
/// （`AttackFrame` 小者先中），帧相同再比射程（`AttackRange` 大者先中）。
///
/// 返回先命中的一方；若帧、射程都相同，返回 `None`——表示进入破势对抗，
/// 由 [`impact_breaks`] 决定是否打断。
pub fn earlier_side(
    first_frame: u32,
    first_range: u32,
    second_frame: u32,
    second_range: u32,
) -> Option<Side> {
    match first_frame.cmp(&second_frame) {
        Ordering::Less => Some(Side::First),
        Ordering::Greater => Some(Side::Second),
        Ordering::Equal => match first_range.cmp(&second_range) {
            Ordering::Greater => Some(Side::First),
            Ordering::Less => Some(Side::Second),
            Ordering::Equal => None,
        },
    }
}

/// 破势对抗机制：完全同刻且双方都命中时，`impact` 大者打断对方攻击
/// （被打断方这次攻击被取消、不掉伤害）；破势相同则互不打断（同时命中）。
pub fn impact_breaks(first_impact: u32, second_impact: u32) -> Option<Side> {
    match first_impact.cmp(&second_impact) {
        Ordering::Greater => Some(Side::Second),
        Ordering::Less => Some(Side::First),
        Ordering::Equal => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn within_range_is_independent_range_check() {
        // 射程 1：距离 1 命中、距离 2 落空
        assert!(within_range(1, 1));
        assert!(!within_range(1, 2));
        // 射程 2：距离 2 仍命中（双方各自独立判定）
        assert!(within_range(2, 2));
    }

    #[test]
    fn both_out_of_range_miss_independently() {
        // 距离 3 超过双方射程 1 → 双方各自落空
        assert!(!within_range(1, 3));
        assert!(!within_range(1, 3));
    }

    #[test]
    fn lower_frame_hits_first() {
        // 帧 5 快于帧 7 → First 先命中
        assert_eq!(earlier_side(5, 1, 7, 1), Some(Side::First));
    }

    #[test]
    fn same_frame_larger_range_hits_first() {
        // 帧相同(5)，射程 2 的长枪先中射程 1 的匕首
        assert_eq!(earlier_side(5, 2, 5, 1), Some(Side::First));
    }

    #[test]
    fn perfect_frame_range_tie_falls_through_to_impact() {
        // 帧、射程全同 → None，交由破势机制
        assert_eq!(earlier_side(5, 1, 5, 1), None);
    }

    #[test]
    fn higher_impact_breaks_lower_one() {
        // 破势 8 > 3 → Second 被打断（若 First/Second 任一方高即打断对方）
        assert_eq!(impact_breaks(8, 3), Some(Side::Second));
        assert_eq!(impact_breaks(3, 8), Some(Side::First));
    }

    #[test]
    fn equal_impact_lets_both_land() {
        // 破势相同 → 不打断，双方同时命中
        assert_eq!(impact_breaks(7, 7), None);
    }

    #[test]
    fn mechanisms_compose_like_systems_would() {
        // 模拟两个同刻攻击：双方射程都够 → 帧定先后；
        // 帧、射程全同才轮到破势打断
        let a_range = 2;
        let b_range = 2;
        let distance = 2;
        assert!(within_range(a_range, distance) && within_range(b_range, distance));
        assert_eq!(earlier_side(5, a_range, 5, b_range), None);
        assert_eq!(impact_breaks(8, 3), Some(Side::Second));
    }
}
