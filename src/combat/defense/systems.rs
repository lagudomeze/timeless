//! 防御域的标记清理、撤销退款与精力回复。
//!
//! 纯逻辑（防御判定 / 反制伤害）住在
//! [`crate::combat::formula::domain`]；系统层只做两件事：
//!
//! 1. 把 ECS 状态翻成纯函数需要的参数（在
//!    [`apply_physical_hits_system`](crate::combat::formula::apply_physical_hits_system) 里）；
//! 2. 清理短命标记（[`expire_defense_markers_system`]）。
//!
//! 「谁能拿回资源」的两条路都是**订阅**：撤销订阅 `ActionCancelled`，
//! 重新可决策订阅 `DecisionReady`——资源归本域，时间线不反向依赖它。

use bevy::prelude::*;

use crate::combat::lifecycle::{Lifetime, Projectile};
use crate::combat::targeting::MeleeShape;
use crate::movement::Velocity;

use super::components::{Dodging, Parrying};
use super::stamina::{STAMINA_REGEN_PER_DECISION, Stamina};

/// 后摇结束、重新可决策时回一点精力（`DecisionReady` → `Stamina`）。
///
/// 「又轮到它决策了」是精力的自然回复点（取代旧模型的「每回合 +1」——
/// 无回合没有回合）。时间线只宣布"槽空了"，回多少、回给谁由资源的拥有者决定。
pub fn recover_stamina_observer(
    ready: On<crate::timeline::DecisionReady>,
    mut units: Query<&mut Stamina>,
) {
    if let Ok(mut stamina) = units.get_mut(ready.entity) {
        stamina.regen(STAMINA_REGEN_PER_DECISION);
    }
}

/// 撤销行动时退还精力（`ActionCancelled` → `Stamina`）。
///
/// 撤销本身归时间线管（它知道"哪条行动还没到点"），**退多少、退给谁**归资源的
/// 拥有者管——这里只负责把广播变成一次 `regen`。
pub fn refund_cancelled_actions_system(
    mut cancelled: MessageReader<crate::timeline::ActionCancelled>,
    mut units: Query<&mut Stamina>,
) {
    for event in cancelled.read() {
        if event.refund == 0 && event.penalty == 0 {
            continue;
        }
        if let Ok(mut stamina) = units.get_mut(event.actor) {
            stamina.regen(event.refund);
            // 取消本身的代价：够就扣，不够就只扣到 0（**不拦着玩家改主意**）
            stamina.try_spend(event.penalty);
        }
    }
}

/// 攻击实体探针：命中任何一类攻击都算「它还在」。
///
/// 覆盖三类攻击实体——射弹（`Projectile` + `Velocity`）、一次性横扫
/// （`MeleeShape` + `Lifetime`）、以及刚落地的火球（`Velocity`）。
/// 漏掉任何一类都会让招架标记在绑定目标还在时被误删。
type AttackProbe = Or<(
    With<Projectile>,
    With<Velocity>,
    With<MeleeShape>,
    With<Lifetime>,
)>;

/// 过期清理：[`Dodging`] 到点移除；[`Parrying`] 到点或绑定的攻击消失即移除。
pub fn expire_defense_markers_system(
    mut commands: Commands,
    now: Res<Time<Virtual>>,
    attacks: Query<(), AttackProbe>,
    dodging: Query<(Entity, &Dodging)>,
    parrying: Query<(Entity, &Parrying)>,
) {
    let now = now.elapsed_secs();
    for (entity, dodging) in &dodging {
        if now >= dodging.expires_at {
            commands.entity(entity).remove::<Dodging>();
        }
    }
    for (entity, parrying) in &parrying {
        let target_gone = attacks.get(parrying.target_attack).is_err();
        if now >= parrying.expires_at || target_gone {
            commands.entity(entity).remove::<Parrying>();
        }
    }
}
