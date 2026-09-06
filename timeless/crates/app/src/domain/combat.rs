//! # 战斗裁决：三层判定（纯 Rust，零引擎依赖）
//!
//! 实现「速度帧 → 距离 → 破势」三层裁决逻辑（双方同刻出手时的先后结算），
//! 迁移自旧 `timeless-domain::combat`。
//!
//! 设计约束：
//! - 本模块**绝不**引入 Bevy / 引擎类型，保证可被 `cargo test` 独立覆盖；
//! - 不包含伤害应用之外的状态变更，纯函数输入 → 输出；
//! - `AttackStats` 是纯逻辑唯一出处，引擎侧用小组件（`AttackFrame` /
//!   `AttackRange` / `Impact` / `Damage`）承载，裁决前组装成该结构传入。

use std::cmp::Ordering;

/// 攻击属性（纯数据）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackStats {
    /// 攻击帧：数值越小，出招越快、越先命中
    pub frame: u32,
    /// 射程（网格距离，按切比雪夫距离计）
    pub range: u32,
    /// 破势值：双方完全同时命中时，破势高者打断对方攻击
    pub impact: u32,
    /// 单次伤害
    pub damage: u32,
}

impl AttackStats {
    pub fn new(frame: u32, range: u32, impact: u32, damage: u32) -> Self {
        Self {
            frame,
            range,
            impact,
            damage,
        }
    }
}

/// 交战双方标识
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Attacker,
    Defender,
}

/// 命中先后顺序（用于输出「谁先命中」）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitOrder {
    AttackerFirst,
    DefenderFirst,
    Simultaneous,
}

/// 一次双方交锋的裁决结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitResult {
    pub attacker_hits: bool,
    pub defender_hits: bool,
    pub order: HitOrder,
    /// 被破势打断的一方：其攻击被取消（不掉伤害）
    pub interrupted: Option<Side>,
}

/// 单方攻击（对方未同时出手，如正在移动/翻滚）：仅做射程判定。
/// 引擎侧在「只有一方提交了 Attack」时调用。
pub fn resolve_attack(attacker: &AttackStats, distance: u32) -> bool {
    distance <= attacker.range
}

/// 双方同时攻击时的三层裁决。
///
/// 裁决链（仅在**双方都命中**时生效）：
/// 1. **速度帧**：`frame` 小者先中（快拳破慢拳）；
/// 2. **距离**：帧相同时，射程大者先中（更长的攻击先触及目标）；
/// 3. **破势**：帧、射程均相同时，`impact` 大者打断对方（对方攻击被取消）；
/// 4. 完全一致 → `Simultaneous`，双方同时造成伤害。
///
/// 若一方在射程外，其攻击直接落空，不进入裁决链。
pub fn resolve_combat(attacker: &AttackStats, defender: &AttackStats, distance: u32) -> HitResult {
    let attacker_hits = distance <= attacker.range;
    let defender_hits = distance <= defender.range;

    // 无接触 / 单方命中：不进入三层裁决链
    if !attacker_hits && !defender_hits {
        return HitResult {
            attacker_hits: false,
            defender_hits: false,
            order: HitOrder::Simultaneous, // 无接触，顺序无意义
            interrupted: None,
        };
    }
    if attacker_hits && !defender_hits {
        return HitResult {
            attacker_hits: true,
            defender_hits: false,
            order: HitOrder::AttackerFirst,
            interrupted: None,
        };
    }
    if !attacker_hits && defender_hits {
        return HitResult {
            attacker_hits: false,
            defender_hits: true,
            order: HitOrder::DefenderFirst,
            interrupted: None,
        };
    }

    // —— 双方都命中：三层裁决链 ——
    // L1: 速度帧
    match attacker.frame.cmp(&defender.frame) {
        Ordering::Less => return HitResult::mutual(HitOrder::AttackerFirst, None),
        Ordering::Greater => return HitResult::mutual(HitOrder::DefenderFirst, None),
        Ordering::Equal => {}
    }
    // L2: 距离（射程大者先中）
    match attacker.range.cmp(&defender.range) {
        Ordering::Greater => return HitResult::mutual(HitOrder::AttackerFirst, None),
        Ordering::Less => return HitResult::mutual(HitOrder::DefenderFirst, None),
        Ordering::Equal => {}
    }
    // L3: 破势（impact 大者打断对方）
    match attacker.impact.cmp(&defender.impact) {
        Ordering::Greater => HitResult::mutual(HitOrder::Simultaneous, Some(Side::Defender)),
        Ordering::Less => HitResult::mutual(HitOrder::Simultaneous, Some(Side::Attacker)),
        Ordering::Equal => HitResult::mutual(HitOrder::Simultaneous, None), // 完全同时，双中
    }
}

impl HitResult {
    fn mutual(order: HitOrder, interrupted: Option<Side>) -> Self {
        Self {
            attacker_hits: true,
            defender_hits: true,
            order,
            interrupted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 基准对局：玩家快而弱破势，敌人慢而强破势
    fn sample() -> (AttackStats, AttackStats) {
        (
            AttackStats::new(5, 1, 3, 10), // 玩家：帧5 射程1 破势3
            AttackStats::new(7, 1, 2, 8),  // 敌人：帧7 射程1 破势2
        )
    }

    #[test]
    fn layer1_speed_frame_decides_who_hits_first() {
        let (p, e) = sample();
        let r = resolve_combat(&p, &e, 1);
        assert_eq!(r.order, HitOrder::AttackerFirst);
        assert!(r.attacker_hits && r.defender_hits);
        assert_eq!(r.interrupted, None);
    }

    #[test]
    fn layer2_larger_range_breaks_frame_tie() {
        // 帧相同(5)，射程不同：射程 2 的长枪先中射程 1 的匕首
        let spear = AttackStats::new(5, 2, 1, 6);
        let dagger = AttackStats::new(5, 1, 4, 12);
        let r = resolve_combat(&spear, &dagger, 1);
        assert_eq!(r.order, HitOrder::AttackerFirst);
        assert_eq!(r.interrupted, None, "L2 只定顺序，不触发打断");
    }

    #[test]
    fn layer3_poise_interrupts_on_full_tie() {
        // 帧、射程全同，破势不同 → 高破势打断低破势
        let heavy = AttackStats::new(5, 1, 8, 5);
        let light = AttackStats::new(5, 1, 3, 9);
        let r = resolve_combat(&heavy, &light, 1);
        assert_eq!(r.order, HitOrder::Simultaneous);
        assert_eq!(r.interrupted, Some(Side::Defender), "轻击被重击打断");
        assert!(r.attacker_hits && r.defender_hits);
    }

    #[test]
    fn perfect_tie_both_land() {
        let a = AttackStats::new(5, 1, 3, 7);
        let b = AttackStats::new(5, 1, 3, 7);
        let r = resolve_combat(&a, &b, 1);
        assert_eq!(r.order, HitOrder::Simultaneous);
        assert_eq!(r.interrupted, None);
        assert!(r.attacker_hits && r.defender_hits);
    }

    #[test]
    fn out_of_range_both_miss() {
        let (p, e) = sample();
        let r = resolve_combat(&p, &e, 3);
        assert!(!r.attacker_hits && !r.defender_hits);
    }

    #[test]
    fn one_side_out_of_range_hits_alone() {
        let (p, _e) = sample();
        // 敌人改为远程（射程2），玩家近战（射程1）在距离2 → 只有敌人命中
        let ranged_enemy = AttackStats::new(3, 2, 1, 5);
        let r = resolve_combat(&p, &ranged_enemy, 2);
        assert!(!r.attacker_hits && r.defender_hits);
        assert_eq!(r.order, HitOrder::DefenderFirst);
    }

    #[test]
    fn single_attack_range_check() {
        let p = AttackStats::new(5, 1, 3, 10);
        assert!(resolve_attack(&p, 1));
        assert!(!resolve_attack(&p, 2));
    }
}
