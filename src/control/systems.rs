//! 消费 `MoveCommand`：为玩家实体设置 `Velocity`
use bevy::prelude::*;

use crate::combat::{Faction, Velocity};

use super::{MoveSpeed, messages::MoveCommand};

/// 对每个 `MoveCommand`：把玩家（`Faction::Player`）的速度设为
/// `axis × MoveSpeed`；零方向自然归零速度。
pub fn apply_move_command_system(
    mut commands: MessageReader<MoveCommand>,
    mut q: Query<(&mut Velocity, &MoveSpeed, &Faction)>,
) {
    for command in commands.read() {
        for (mut velocity, speed, faction) in &mut q {
            if *faction == Faction::Player {
                velocity.0 = command.axis.extend(0.0) * speed.0;
            }
        }
    }
}
