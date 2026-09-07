use bevy::prelude::*;

#[derive(Event)]
pub struct DamageEvent {
    pub target: Entity,
    pub amount: f32,
}

#[derive(Event)]
pub struct DeathEvent {
    pub entity: Entity,
}

#[derive(Component)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

pub fn apply_damage(
    trigger: On<DamageEvent>,
    mut health_query: Query<&mut Health>,
    mut commands: Commands,
) {
    let event = trigger.event();
    if let Ok(mut health) = health_query.get_mut(event.target) {
        health.current -= event.amount;
        if health.current <= 0.0 {
            commands.trigger(DeathEvent {
                entity: event.target,
            });
        }
    }
}
