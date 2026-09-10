//! 战斗重置——一个**功能**，不是一个领域。
//!
//! 它没有自己的数据模型：只是「清场 + 用同一套工厂重新组装」这段功能胶水，
//! 所以既不归 ECS 数据域，也不归表现域，跟着组装车间走。
//! 触发键（R）与消费系统同属本文件：功能自己负责翻译按键、自己负责落地。

use bevy::prelude::*;

use crate::combat::{Collidable, Faction, Projectile};
use crate::timeline::{ScheduledAction, Timeline};
use crate::world::TerrainConfig;

use super::enemy::enemy_scene;
use super::player::player_scene;

/// 请求重置战斗（写：R 键；消费：[`reset_battle_system`]）。
#[derive(Message, Debug, Clone, Copy)]
pub struct ResetBattle;

/// R 键 → [`ResetBattle`]（只翻译，不改状态）。
pub fn restart_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: MessageWriter<ResetBattle>,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        commands.write(ResetBattle);
    }
}

/// 清场目标：单位（`Faction` / `Collidable`）、攻击实体（`Projectile`）
/// 与时间线上没执行的行动（`ScheduledAction`）。
type ResetQuery<'w, 's> = Query<
    'w,
    's,
    Entity,
    Or<(
        With<Faction>,
        With<Projectile>,
        With<Collidable>,
        With<ScheduledAction>,
    )>,
>;

/// 清掉所有单位与攻击实体，再用同一组工厂重建：状态自然回到初始值。
pub fn reset_battle_system(
    mut reset_requests: MessageReader<ResetBattle>,
    mut commands: Commands,
    terrain: Res<TerrainConfig>,
    mut timeline: ResMut<Timeline>,
    entities: ResetQuery<'_, '_>,
) {
    if reset_requests.read().next().is_none() {
        return;
    }
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    timeline.restart(); // 回到规划阶段：虚拟时间重新冻结，等待玩家提交
    commands.spawn_scene(player_scene(&terrain));
    commands.spawn_scene(enemy_scene(&terrain));
    info!("🔄 战斗已重置");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::headless_app;

    fn units(app: &mut App) -> Vec<Faction> {
        let mut query = app.world_mut().query::<&Faction>();
        query.iter(app.world()).copied().collect()
    }

    /// 重置功能：清场后用同一套工厂把玩家与敌人重新组装回来。
    #[test]
    fn reset_rebuilds_units_from_the_same_factories() {
        let mut app = headless_app();
        app.update(); // Startup 组装：玩家 + 敌人

        assert_eq!(units(&mut app).len(), 2, "开局应组装出玩家与敌人");

        // 模拟敌方阵亡：清掉敌人后场上只剩玩家
        let enemy = app
            .world_mut()
            .query_filtered::<Entity, With<Faction>>()
            .iter(app.world())
            .find(|entity| matches!(app.world().get::<Faction>(*entity), Some(Faction::Enemy)))
            .unwrap();
        app.world_mut().entity_mut(enemy).despawn();
        app.update();
        assert_eq!(units(&mut app).len(), 1);

        // R 键请求重置 → 清场重建
        app.world_mut().write_message(ResetBattle);
        app.update();
        app.update();

        let rebuilt = units(&mut app);
        assert_eq!(rebuilt.len(), 2, "重置后应重新组装出玩家与敌人");
        assert!(rebuilt.contains(&Faction::Player) && rebuilt.contains(&Faction::Enemy));
    }
}
