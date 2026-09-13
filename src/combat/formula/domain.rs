//! 裁决纯逻辑（**零 Bevy 依赖**，可脱离 App 单测）。
//!
//! 从早期原型的网格裁决迁入，并按本项目的坐标模型改写：
//! 旧模型用「逻辑刻度 + 格距离」，现在用**真实距离**（世界单位）——
//! 三层裁决的比较维度不变，只是第二层从「格」变成「米」：
//!
//! | 层 | 比较 | 含义 |
//! | :--- | :--- | :--- |
//! | L1 | `AttackFrame` 小者先 | 出手快慢（帧） |
//! | L2 | `range` 大者先 | 同帧时长兵器先中（世界单位） |
//! | L3 | `Impact` 大者打断小者 | 破势 |
//!
//! 这里是**纯函数**：输入两个 [`AttackStats`] 与真实距离，输出 [`HitResult`]。
//! 不引用任何 Bevy 类型；防御状态用本模块的 [`DefenseState`] 表达，
//! 实体身份用 `u64` 折值（`Entity::to_bits()`），由系统层装配后传进来。

use crate::combat::defense::DefenseOutcome;

/// 本模块的最小防御状态：零 Bevy 依赖，因此不直接引用 ECS 组件。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DefenseState {
    /// 是否在无敌帧内
    pub dodging: bool,
    /// 是否处于招架姿态
    pub parrying: bool,
}

/// 一次攻击的裁决参数（纯数据）。
///
/// 由 ECS 侧从组件装配（`AttackFrame` / `AttackRange` / `Impact` / `PhysicalDamage`），
/// 因此公式完全不认识 Bevy。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttackStats {
    /// 速度帧：越小越先命中
    pub frame: u32,
    /// 触及距离（世界单位，便于与真实距离直接比较）
    pub range: f32,
    /// 破势：全同时打断对方
    pub impact: u32,
    /// 伤害
    pub damage: f32,
}

impl AttackStats {
    pub fn new(frame: u32, range: f32, impact: u32, damage: f32) -> Self {
        Self {
            frame,
            range,
            impact,
            damage,
        }
    }

    /// 这次攻击在 `distance`（真实距离）处是否够得着。
    pub fn reaches(&self, distance: f32) -> bool {
        distance <= self.range
    }
}

/// 谁被打断。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Attacker,
    Defender,
}

/// 出手顺序。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum HitOrder {
    #[default]
    AttackerFirst,
    DefenderFirst,
    Simultaneous,
}

/// 一次对砍的裁决结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitResult {
    /// 先手方能否命中
    pub attacker_hits: bool,
    /// 后手方能否命中
    pub defender_hits: bool,
    /// 谁先命中
    pub order: HitOrder,
    /// 被破势打断的一方（`None` = 双方都打出去）
    pub interrupted: Option<Side>,
}

impl HitResult {
    /// 单方攻击（对手没有还手）：只看够不够得着。
    pub fn mutual(attacker_hits: bool, defender_hits: bool, order: HitOrder) -> Self {
        Self {
            attacker_hits,
            defender_hits,
            order,
            interrupted: None,
        }
    }
}

/// 单方攻击的射程判定。
pub fn resolve_attack(stats: &AttackStats, distance: f32) -> bool {
    stats.reaches(distance)
}

/// 一次防御判定的纯逻辑：谁在什么时候有效。
///
/// 实体用 **`u64` 索引**（`Entity::index()` 的产物）而不是 `Entity` 本身，
/// 这样本模块不需要 `use bevy::prelude::*`，保持零 Bevy 依赖。
pub fn resolve_defense(
    defense: DefenseState,
    attack_entity: u64,
    parry_target: Option<u64>,
) -> DefenseOutcome {
    if defense.dodging {
        return DefenseOutcome::Dodged;
    }
    if defense.parrying && parry_target == Some(attack_entity) {
        return DefenseOutcome::Parried;
    }
    DefenseOutcome::Landed
}

/// 招架反制伤害：招架成功时回敬对方一半伤害（向上取整，至少 1 点）。
pub fn counter_damage(incoming: f32) -> f32 {
    if incoming <= 0.0 {
        return 0.0;
    }
    (incoming / 2.0).ceil().max(1.0)
}

/// 三层裁决：帧 → 距离 → 破势。
///
/// `distance` 是双方之间的**真实距离**（世界单位）。
pub fn resolve_combat(attacker: &AttackStats, defender: &AttackStats, distance: f32) -> HitResult {
    let attacker_reaches = attacker.reaches(distance);
    let defender_reaches = defender.reaches(distance);

    // 只有一方够得着：直接判定
    if !defender_reaches {
        return HitResult::mutual(attacker_reaches, false, HitOrder::AttackerFirst);
    }
    if !attacker_reaches {
        return HitResult::mutual(false, defender_reaches, HitOrder::DefenderFirst);
    }

    // L1：帧
    let order = match attacker.frame.cmp(&defender.frame) {
        std::cmp::Ordering::Less => HitOrder::AttackerFirst,
        std::cmp::Ordering::Greater => HitOrder::DefenderFirst,
        // L2：同帧比距离（够得着的距离更长 = 长兵器先中）
        std::cmp::Ordering::Equal => match attacker.range.total_cmp(&defender.range) {
            std::cmp::Ordering::Greater => HitOrder::AttackerFirst,
            std::cmp::Ordering::Less => HitOrder::DefenderFirst,
            // L3：距离也相同 → 同时出手，由破势决定谁被打断
            std::cmp::Ordering::Equal => HitOrder::Simultaneous,
        },
    };

    if order != HitOrder::Simultaneous {
        // 先手方先落地；同刻双方都能命中（不存在「后手被取消」）
        return HitResult::mutual(true, true, order);
    }

    // 同时命中：破势高者打断低者
    let interrupted = match attacker.impact.cmp(&defender.impact) {
        std::cmp::Ordering::Greater => Some(Side::Defender),
        std::cmp::Ordering::Less => Some(Side::Attacker),
        std::cmp::Ordering::Equal => None,
    };
    HitResult {
        attacker_hits: true,
        defender_hits: true,
        order,
        interrupted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 基准对局：玩家快而破势低，敌人慢而破势高（世界单位距离 2.0）。
    fn sample() -> (AttackStats, AttackStats) {
        (
            AttackStats::new(5, 3.0, 3, 10.0),
            AttackStats::new(7, 3.0, 8, 8.0),
        )
    }

    #[test]
    fn layer1_speed_frame_decides_who_hits_first() {
        let (player, enemy) = sample();
        let result = resolve_combat(&player, &enemy, 2.0);
        assert_eq!(result.order, HitOrder::AttackerFirst);
        assert!(result.attacker_hits && result.defender_hits);
        assert_eq!(result.interrupted, None);
    }

    #[test]
    fn layer2_longer_reach_breaks_frame_tie() {
        let spear = AttackStats::new(5, 6.0, 1, 6.0);
        let dagger = AttackStats::new(5, 2.0, 4, 12.0);
        let result = resolve_combat(&spear, &dagger, 2.0);
        assert_eq!(result.order, HitOrder::AttackerFirst, "同帧时长兵器先中");
        assert_eq!(result.interrupted, None, "L2 只定顺序，不触发打断");
    }

    #[test]
    fn layer3_poise_interrupts_on_full_tie() {
        let heavy = AttackStats::new(5, 3.0, 8, 5.0);
        let light = AttackStats::new(5, 3.0, 3, 9.0);
        let result = resolve_combat(&heavy, &light, 2.0);
        assert_eq!(result.order, HitOrder::Simultaneous);
        assert_eq!(result.interrupted, Some(Side::Defender), "轻击被重击打断");
        assert!(result.attacker_hits && result.defender_hits);
    }

    #[test]
    fn perfect_tie_both_land() {
        let a = AttackStats::new(5, 3.0, 3, 7.0);
        let b = AttackStats::new(5, 3.0, 3, 7.0);
        let result = resolve_combat(&a, &b, 2.0);
        assert_eq!(result.order, HitOrder::Simultaneous);
        assert_eq!(result.interrupted, None);
        assert!(result.attacker_hits && result.defender_hits);
    }

    #[test]
    fn out_of_range_both_miss() {
        let (player, enemy) = sample();
        let result = resolve_combat(&player, &enemy, 30.0);
        assert!(!result.attacker_hits && !result.defender_hits);
    }

    #[test]
    fn one_side_out_of_range_hits_alone() {
        let melee = AttackStats::new(5, 2.0, 3, 10.0);
        let ranged = AttackStats::new(3, 12.0, 1, 5.0);
        let result = resolve_combat(&melee, &ranged, 6.0);
        assert!(!result.attacker_hits && result.defender_hits);
        assert_eq!(result.order, HitOrder::DefenderFirst);
    }

    #[test]
    fn single_attack_range_check() {
        let attack = AttackStats::new(5, 4.0, 3, 10.0);
        assert!(resolve_attack(&attack, 3.9));
        assert!(!resolve_attack(&attack, 4.1));
    }

    #[test]
    fn range_boundary_is_inclusive() {
        let attack = AttackStats::new(5, 4.0, 3, 10.0);
        assert!(
            resolve_attack(&attack, 4.0),
            "距离正好等于射程算够得着（与 A 的碰撞判定一致）"
        );
    }

    #[test]
    fn dodge_beats_parry_and_landing() {
        let defense = DefenseState {
            dodging: true,
            parrying: true,
        };
        assert_eq!(
            resolve_defense(defense, 1, Some(1)),
            DefenseOutcome::Dodged,
            "在无敌帧里就是闪开了，不需要看招架"
        );
    }

    #[test]
    fn parry_only_negates_the_bound_attack() {
        let defense = DefenseState {
            dodging: false,
            parrying: true,
        };
        assert_eq!(
            resolve_defense(defense, 7, Some(7)),
            DefenseOutcome::Parried
        );
        assert_eq!(
            resolve_defense(defense, 9, Some(7)),
            DefenseOutcome::Landed,
            "招架只挡绑定的那一次攻击"
        );
    }

    #[test]
    fn counter_damage_halves_and_never_rounds_to_zero() {
        assert_eq!(counter_damage(10.0), 5.0);
        assert_eq!(counter_damage(15.0), 8.0, "向上取整");
        assert_eq!(counter_damage(1.0), 1.0, "至少 1 点");
        assert_eq!(counter_damage(0.0), 0.0);
    }
}
