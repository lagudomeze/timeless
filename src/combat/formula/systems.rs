//! 物理伤害计算。
//!
//! 这里只有**纯公式**：裁决与结算在 [`crate::combat::formula::resolution`]
//! （`phase1_arbitrate_system` 只读裁决 → `phase2_apply_system` 统一落地）——
//! 防御判定必须先于护甲计算，因此「什么时候算伤害」归裁决，「算多少」归本域。

/// 物理伤害公式：原始伤害扣护甲，最低为 0。
///
/// 纯函数（零 Bevy 依赖），公式可以单独单测；新增减免机制时在这里扩展。
pub fn physical_damage(raw: f32, armor: f32) -> f32 {
    (raw - armor).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::formula::DamageType;

    #[test]
    fn armour_reduces_physical_damage_but_never_below_zero() {
        assert_eq!(physical_damage(10.0, 0.0), 10.0);
        assert_eq!(physical_damage(10.0, 3.0), 7.0);
        assert_eq!(physical_damage(10.0, 30.0), 0.0, "护甲超过伤害时不产生负数");
    }

    #[test]
    fn physical_is_the_default_damage_type() {
        assert_eq!(DamageType::default(), DamageType::Physical);
    }
}
