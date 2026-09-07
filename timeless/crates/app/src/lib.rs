use bevy::prelude::*;

mod ai;
pub mod attacks;
mod camera;
mod character;
pub mod combat;
pub mod control;
mod decoration;
pub mod despawn;
pub mod events;
pub mod health;
mod map;

pub fn preload(mut commands: Commands) {
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 400.0,
        ..default()
    });
    decoration::load_natures(&mut commands);
}

pub fn setup(mut commands: Commands, natures: Res<decoration::Natures>) {
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, 0.8, 0.0)),
    ));

    commands.spawn_scene(camera::main_camera());
    commands.spawn_scene(camera::light());
    commands.spawn_scene_list(bsn_list! {
        (
            @map::Ground {
                @width : 10.0,
                @height : 10.0,
            }
        ),
        (
            @map::GroundGrid {
                @width : 10.0,
                @height : 10.0,
            }
        )
    });
    commands.spawn_scene(character::player());
    commands.spawn_scene(character::enemy());

    for x in 0..5 {
        for y in 0..5 {
            let transform = Transform::from_xyz(x as f32, 0.0, y as f32);
            commands.spawn_scene(natures.random(transform));
        }
    }
}

/// 装配战斗流水线（蓝图系统链，顺序不可乱）：
/// 移动 → 碰撞检测 → 伤害计算 → 命中计数 → 扣血 → 清理 → 死亡销毁。
pub fn add_combat(app: &mut App) {
    app.add_message::<events::DamageEvent>()
        .add_message::<events::DeathEvent>()
        .add_message::<control::MoveCommand>()
        .add_message::<attacks::FireCommand>()
        .add_systems(
            Update,
            (
                control::player_move_input_system,
                ai::enemy_ai_system,
                attacks::player_fire_input_system,
                attacks::player_fire_arrow_system,
                control::apply_move_command_system,
                combat::move_entities_system,
                combat::detect_collisions_system,
                combat::apply_physical_damage_system,
                combat::manage_projectile_hits_system,
                health::apply_damage_system,
                combat::cleanup_finished_attacks_system,
                despawn::despawn_dead_system,
            )
                .chain(),
        );
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::keyboard::KeyCode;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(ButtonInput::<KeyCode>::default());
        add_combat(&mut app);
        app
    }

    #[test]
    fn arrow_damages_target_then_is_cleaned_up() {
        let mut app = test_app();
        let target = app
            .world_mut()
            .spawn((
                health::Health::new(100.0),
                combat::Collidable,
                combat::HitRadius(0.8),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id();
        let arrow = app
            .world_mut()
            .spawn((
                combat::Velocity(Vec3::ZERO),
                combat::Projectile::default(),
                combat::HitRadius(0.2),
                combat::PhysicalDamage(10.0),
                Transform::from_xyz(0.5, 0.0, 0.0),
            ))
            .id();

        app.update(); // 碰撞挂标记
        app.update(); // 伤害 → 命中结束 → 清理

        let world = app.world_mut();
        assert!(
            world.get_entity(arrow).is_err(),
            "普通射弹（穿透 1）命中后应被清理"
        );
        let hp = world.query::<&health::Health>().get(world, target).unwrap();
        assert_eq!(hp.current, 90.0, "10 点物理伤害应扣减 10 点生命");
    }

    #[test]
    fn armor_reduces_physical_damage() {
        let mut app = test_app();
        let target = app
            .world_mut()
            .spawn((
                health::Health::new(100.0),
                combat::Collidable,
                combat::HitRadius(0.8),
                combat::Armor(3.0),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id();
        app.world_mut().spawn((
            combat::Velocity(Vec3::ZERO),
            combat::Projectile::default(),
            combat::HitRadius(0.2),
            combat::PhysicalDamage(10.0),
            Transform::from_xyz(0.5, 0.0, 0.0),
        ));

        app.update();
        app.update();

        let world = app.world_mut();
        let hp = world.query::<&health::Health>().get(world, target).unwrap();
        assert_eq!(hp.current, 93.0, "10 点物理伤害应被 3 点护甲减免");
    }

    #[test]
    fn lethal_damage_triggers_despawn() {
        let mut app = test_app();
        let target = app
            .world_mut()
            .spawn((
                health::Health::new(5.0),
                combat::Collidable,
                combat::HitRadius(0.8),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id();
        app.world_mut().spawn((
            combat::Velocity(Vec3::ZERO),
            combat::Projectile::default(),
            combat::HitRadius(0.2),
            combat::PhysicalDamage(10.0),
            Transform::from_xyz(0.5, 0.0, 0.0),
        ));

        app.update();
        app.update();

        assert!(
            app.world_mut().get_entity(target).is_err(),
            "致命伤害应触发 DeathEvent 并销毁目标"
        );
    }

    #[test]
    fn wasd_moves_player_through_move_command() {
        let mut app = test_app();
        app.world_mut().spawn((
            combat::Faction::Player,
            combat::Velocity(Vec3::ZERO),
            control::MoveSpeed(5.0),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);

        app.update();

        let world = app.world_mut();
        let velocity = world.query::<&combat::Velocity>().single(world).unwrap().0;
        assert_eq!(velocity, Vec3::Y * 5.0, "按住 W 应给玩家 +Y 速度");
    }
}
