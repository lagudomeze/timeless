//! 防御与反制的纯逻辑（**零 Bevy 依赖**，可脱离 App 单测）。
//!
//! 这里只剩两件事：**这一击有没有被挡下**、**挡下了要回敬多少**。
//! 三层裁决（帧 → 距离 → 破势）随两阶段结算一起删掉了：无回合模型里"谁先出手"
//! 由各自的 `execute_at` 决定，同刻相撞不再需要一份快照来仲裁；而"打断"改由
//! [`InterruptEvent`](crate::timeline::InterruptEvent) 针对**还没到点的行动**做对抗
//! （打的是"还没发生的事"，不是"已经打出来的一击"）。
//!
//! 实体身份用 `u64` 折值（`Entity::to_bits()`），由系统层装配后传进来，
//! 因此本模块不引用任何 Bevy 类型。

use crate::combat::defense::DefenseOutcome;

/// 本模块的最小防御状态：零 Bevy 依赖，因此不直接引用 ECS 组件。
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct DefenseState {
    /// 是否在无敌帧内
    pub dodging: bool,
    /// 是否处于招架姿态
    pub parrying: bool,
    /// 格挡率（0..=1）；`0` = 没有格挡能力
    pub block_chance: f32,
}

/// 一次防御判定的纯逻辑：谁在什么时候有效。
///
/// 优先级：无敌帧 > 招架 > 格挡 > 照常吃伤害——**顺序即设计**
/// （`docs/combat.md` 第一节的六关）。招架只挡**绑定的那一次**攻击；
/// 格挡是减伤不是免伤，所以它**不改变"吃到了冲击"**这件事。
///
/// `block_roll` 由调用方掷好传进来（本模块零随机）。格挡率 `0` 时**连骰子都不用看**。
pub fn resolve_defense(
    defense: DefenseState,
    attack_entity: u64,
    parry_target: Option<u64>,
    block_roll: f32,
) -> DefenseOutcome {
    if defense.dodging {
        return DefenseOutcome::Dodged;
    }
    if defense.parrying && parry_target == Some(attack_entity) {
        return DefenseOutcome::Parried;
    }
    if let Some(absorbed) = resolve_block(defense.block_chance, block_roll) {
        return DefenseOutcome::Blocked { absorbed };
    }
    DefenseOutcome::Landed
}

/// 格挡判定：**掷一次骰，看落不落在格挡率里**。
///
/// 与闪避 / 招架一样是**设计可控**（格挡率由装备给），但结果是**减伤不是免伤**。
/// 骰子由调用方掷好传进来——本模块零 Bevy、也零随机，边界才好单测。
///
/// 返回 `Some(absorbed)` 表示挡下了，`absorbed` 是减伤比例（0..1，向 1 靠）；
/// `None` 表示没挡住，照常吃伤害。
pub fn resolve_block(chance: f32, roll: f32) -> Option<f32> {
    let chance = chance.clamp(0.0, 1.0);
    let roll = roll.clamp(0.0, 1.0);
    (roll < chance).then_some(chance)
}

/// 格挡之后的伤害：**减伤，不是归零**。
pub fn blocked_damage(raw: i32, absorbed: f32) -> i32 {
    if raw <= 0 {
        return 0;
    }
    let kept = raw as f32 * (1.0 - absorbed.clamp(0.0, 1.0));
    // 向 0 取整：挡下 30% 的 10 点是 7 点，不是 7.0 点（伤害是整数）
    kept.floor().max(0.0) as i32
}

/// 招架反制伤害：招架成功时回敬对方一半伤害（向上取整，至少 1 点）。
pub fn counter_damage(incoming: i32) -> i32 {
    if incoming <= 0 {
        return 0;
    }
    ((incoming + 1) / 2).max(1)
}

/// 打断对抗的底数：双方各加这么多，再各加 3d5。
///
/// 它在不等式两边同时出现，**对结果没有影响**（会被约掉）——留着是因为设计稿
/// 那行算式就是这么写的，读起来对得上。想让"打断更难一点"，改这里是个陷阱，
/// 真正的手感旋钮是 `ActionTiming::interrupt_resist` 与 `InterruptPower`。
pub const INTERRUPT_BASE: i32 = 3;

/// 一次打断对抗的纯逻辑：攻方 `power` 对守方 `resist`，骰子由调用方掷好传进来
/// （本模块零 Bevy、也零随机，方便单测边界）。
///
/// ```text
/// 攻方 = power  + INTERRUPT_BASE + attack_roll
/// 守方 = resist + INTERRUPT_BASE + defense_roll
/// 攻方 >= 守方 → 这一手被打掉
/// ```
pub fn interrupt_lands(
    attack_power: i32,
    defense_resist: i32,
    attack_roll: i32,
    defense_roll: i32,
) -> bool {
    attack_power + INTERRUPT_BASE + attack_roll >= defense_resist + INTERRUPT_BASE + defense_roll
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dodge_beats_parry_and_landing() {
        let defense = DefenseState {
            dodging: true,
            parrying: true,
            block_chance: 0.0,
        };
        assert_eq!(
            resolve_defense(defense, 1, Some(1), 0.0),
            DefenseOutcome::Dodged,
            "在无敌帧里就是闪开了，不需要看招架"
        );
    }

    #[test]
    fn parry_only_negates_the_bound_attack() {
        let defense = DefenseState {
            dodging: false,
            parrying: true,
            block_chance: 0.0,
        };
        assert_eq!(
            resolve_defense(defense, 7, Some(7), 0.0),
            DefenseOutcome::Parried
        );
        assert_eq!(
            resolve_defense(defense, 9, Some(7), 0.0),
            DefenseOutcome::Landed,
            "招架只挡绑定的那一次攻击"
        );
    }

    #[test]
    fn an_idle_target_takes_the_hit() {
        assert_eq!(
            resolve_defense(DefenseState::default(), 3, None, 0.0),
            DefenseOutcome::Landed
        );
    }

    /// **格挡在第三关、闪避与招架之后**——顺序即设计（`docs/combat.md` 第一节）。
    #[test]
    fn blocking_comes_after_dodge_and_parry() {
        // 能闪就闪：格挡率再高也轮不到它
        let dodging = DefenseState {
            dodging: true,
            parrying: false,
            block_chance: 1.0,
        };
        assert_eq!(
            resolve_defense(dodging, 1, None, 0.0),
            DefenseOutcome::Dodged,
            "闪避压过格挡"
        );

        // 招架同理（它挡的是**绑定**的那一次）
        let parrying = DefenseState {
            dodging: false,
            parrying: true,
            block_chance: 1.0,
        };
        assert_eq!(
            resolve_defense(parrying, 1, Some(1), 0.0),
            DefenseOutcome::Parried,
            "招架压过格挡"
        );

        // 两者都不在，才轮到格挡
        let blocking = DefenseState {
            dodging: false,
            parrying: false,
            block_chance: 0.5,
        };
        assert_eq!(
            resolve_defense(blocking, 1, None, 0.2),
            DefenseOutcome::Blocked { absorbed: 0.5 },
            "骰子落在格挡率之内 → 挡下"
        );
        assert_eq!(
            resolve_defense(blocking, 1, None, 0.7),
            DefenseOutcome::Landed,
            "骰子落在格挡率之外 → 没挡住"
        );
    }

    /// 格挡是**减伤不是免伤**：挡下之后仍然有伤害，且仍然算"吃到了冲击"。
    #[test]
    fn blocking_reduces_damage_instead_of_negating_it() {
        assert_eq!(blocked_damage(10, 0.3), 7, "挡掉 30% → 剩 7 点");
        assert_eq!(blocked_damage(10, 0.0), 10, "没挡掉任何");
        assert_eq!(blocked_damage(10, 1.0), 0, "百分百格挡才归零");
        assert_eq!(blocked_damage(0, 0.5), 0, "零伤害不参与计算");
    }

    /// 格挡率 `0` 时**连骰子都不看**（`roll` 传什么都不该挡下）。
    #[test]
    fn a_zero_block_chance_never_blocks() {
        let no_block = DefenseState {
            dodging: false,
            parrying: false,
            block_chance: 0.0,
        };
        for roll in [0.0, 0.5, 0.99] {
            assert_eq!(
                resolve_defense(no_block, 1, None, roll),
                DefenseOutcome::Landed,
                "格挡率为 0 时 roll={roll} 也不该挡下"
            );
        }
    }

    /// 越界的格挡率会被夹住：设计数据写错不该把伤害算成负数。
    #[test]
    fn an_out_of_range_block_chance_is_clamped() {
        assert_eq!(resolve_block(1.5, 0.99), Some(1.0), "夹到 1.0");
        assert_eq!(resolve_block(-0.5, 0.0), None, "夹到 0.0 → 永远挡不下");
        assert_eq!(blocked_damage(10, -1.0), 10, "负的减伤 = 全额吃下");
    }

    #[test]
    fn counter_damage_halves_and_never_rounds_to_zero() {
        assert_eq!(counter_damage(10), 5);
        assert_eq!(counter_damage(15), 8, "向上取整");
        assert_eq!(counter_damage(1), 1, "至少 1 点");
        assert_eq!(counter_damage(0), 0);
        assert_eq!(counter_damage(-5), 0, "负伤害不产生反制");
    }

    /// 打断对抗：平手算攻方赢（"攻方 >= 守方"），差 1 点就反过来了。
    #[test]
    fn an_interrupt_lands_on_a_tie() {
        assert!(interrupt_lands(3, 3, 0, 0), "平手归攻方");
        assert!(
            interrupt_lands(2, 3, 5, 4),
            "骰子差能补上力度差（2+5 >= 3+4）"
        );
        assert!(
            !interrupt_lands(2, 3, 4, 4),
            "差一点点就是没打断（2+4 < 3+4）"
        );
    }

    /// 那 3 点底数在不等式两边同时出现，**不影响结果**。
    #[test]
    fn the_interrupt_base_cancels_out() {
        assert_eq!(
            interrupt_lands(3, 3, 0, 0),
            interrupt_lands(0, 0, 0, 0),
            "两边同加一个常数，结果不变"
        );
    }
}
