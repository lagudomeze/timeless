//! 重置系统：清场后按单位场景工厂重新生成玩家与敌人
use bevy::prelude::*;

use crate::character;
use crate::combat::{Collidable, Faction, Projectile};

/// 清场目标：单位（`Faction` / `Collidable`）与攻击实体（`Projectile`）
type ResetQuery<'w, 's> =
    Query<'w, 's, Entity, Or<(With<Faction>, With<Projectile>, With<Collidable>)>>;

/// 重置战斗请求（由 `reset_input_system` 写入、`reset_system` 消费）
#[derive(Message, Debug, Clone, Copy)]
pub struct ResetBattle;

/// 清掉所有单位（`Faction` / `Collidable`）与攻击实体（`Projectile`），
/// 再用同一组场景工厂重建，状态自然回到初始值。
pub fn reset_system(
    mut reset_requests: MessageReader<ResetBattle>,
    mut commands: Commands,
    entities: ResetQuery<'_, '_>,
) {
    if reset_requests.read().next().is_none() {
        return;
    }
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    commands.spawn_scene(character::player());
    commands.spawn_scene(character::enemy());
    info!("🔄 战斗已重置");
}
