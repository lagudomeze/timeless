//! 战斗重置——一个**功能**，不是一个领域。
//!
//! 它没有自己的数据模型：只是「清场 + 用同一套工厂重新组装」这段功能胶水，
//! 所以既不归 ECS 数据域，也不归表现域，跟着组装车间走。
//!
//! **不认识按键**：触发键（`F5`）住在 [`crate::input`]，那里只把它翻成
//! [`ResetBattle`]；本文件只消费这条消息。

use bevy::prelude::*;

use crate::combat::{Collidable, Faction, Projectile};
use crate::presentation::UnitSprites;
use crate::world::TerrainConfig;

use super::enemy::enemy_scene;
use super::player::player_scene;

/// 请求重置战斗（写：[`crate::input`] 的 F5；消费：[`reset_battle_system`]）。
#[derive(Message, Debug, Clone, Copy)]
pub struct ResetBattle;

/// 清场目标：单位（`Faction` / `Collidable`）与攻击实体（`Projectile`）。
///
/// 时间线上没执行的行动**不用单独列**：它们归行动者所有，人没了行动跟着没
/// （`ActionOf` 的 `Actions` 是 linked spawn）。反过来若把 `With<ScheduledAction>`
/// 也写进来，同一个实体就会被 despawn 两次。
type ResetQuery<'w, 's> =
    Query<'w, 's, Entity, Or<(With<Faction>, With<Projectile>, With<Collidable>)>>;

/// 清掉所有单位与攻击实体，再用同一组工厂重建：状态自然回到初始值。
///
/// 无回合模型下不需要「回到规划阶段」——重建出来的单位决策槽是空的，
/// 时间线的计算系统下一帧自会重新判断该不该停表。
pub fn reset_battle_system(
    mut reset_requests: MessageReader<ResetBattle>,
    mut commands: Commands,
    terrain: Res<TerrainConfig>,
    sprites: Res<UnitSprites>,
    entities: ResetQuery<'_, '_>,
) {
    if reset_requests.read().next().is_none() {
        return;
    }
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    commands.spawn_scene(player_scene(&terrain, &sprites));
    commands.spawn_scene(enemy_scene(&terrain, &sprites));
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

        // F5 键请求重置 → 清场重建
        app.world_mut().write_message(ResetBattle);
        app.update();
        app.update();

        let rebuilt = units(&mut app);
        assert_eq!(rebuilt.len(), 2, "重置后应重新组装出玩家与敌人");
        assert!(rebuilt.contains(&Faction::Player) && rebuilt.contains(&Faction::Enemy));
    }
}
