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
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DefenseState {
    /// 是否在无敌帧内
    pub dodging: bool,
    /// 是否处于招架姿态
    pub parrying: bool,
}

/// 一次防御判定的纯逻辑：谁在什么时候有效。
///
/// 优先级：无敌帧 > 招架 > 照常吃伤害。招架只挡**绑定的那一次**攻击。
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
pub fn counter_damage(incoming: i32) -> i32 {
    if incoming <= 0 {
        return 0;
    }
    ((incoming + 1) / 2).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn an_idle_target_takes_the_hit() {
        assert_eq!(
            resolve_defense(DefenseState::default(), 3, None),
            DefenseOutcome::Landed
        );
    }

    #[test]
    fn counter_damage_halves_and_never_rounds_to_zero() {
        assert_eq!(counter_damage(10), 5);
        assert_eq!(counter_damage(15), 8, "向上取整");
        assert_eq!(counter_damage(1), 1, "至少 1 点");
        assert_eq!(counter_damage(0), 0);
        assert_eq!(counter_damage(-5), 0, "负伤害不产生反制");
    }
}
