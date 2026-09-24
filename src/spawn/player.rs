//! 玩家组装：单位零件 + 玩家专属零件 + 装备槽位。
//!
//! 驱动力来自 [`crate::input`]（键盘 → `MoveCommand` → movement 落地）；
//! 玩家额外兼任区块加载器（[`ChunkLoader`]），走哪儿加载哪儿。

use bevy::prelude::*;

use crate::combat::Faction;
use crate::equipment::{ItemKind, SlotKind, equip, equipment_slot_scene};
use crate::movement::{Cell, MoveSpeed};
use crate::presentation::unit_sprite::UnitSprites;
use crate::timeline::InputDriven;
use crate::world::{ChunkLoader, TerrainConfig};

use super::cell_ground;
use super::unit::unit_scene;

/// 玩家出生格（镜头也跟着它走，见 `presentation::camera`）。
///
/// 取 (1,0) 而不是 (1,1)：与敌人 (3,3) 的**格中心**间距保持约 3.6 格，
/// 和改动前（世界 (2,2) → (7,7) = 7.1 单位）基本一致。
pub const PLAYER_SPAWN: Cell = Cell::new(1, 0);

/// 玩家场景：骑士精灵（后续换一张贴图即可换角色，逻辑零件不动）。
///
/// `InputDriven` 是**输入归属**标记：时间线靠它决定「世界该停下来等谁」，
/// 反应系统靠它决定「谁被威胁」、「谁能用 Focus 换前摇」。
/// 它与 `Faction` 分工不同：`Faction` 管战斗目标过滤，两者互不替代。
pub fn player_scene(terrain: &TerrainConfig, sprites: &UnitSprites) -> impl Scene {
    let position = cell_ground(terrain, PLAYER_SPAWN);
    let loader = ChunkLoader::default();
    bsn! {
        unit_scene(Faction::Player, position, sprites)
        InputDriven
        MoveSpeed(5.0)
        template_value(loader)
    }
}

/// 给玩家挂上装备槽位与**起始装备**（组装层是决定"谁身上有什么"的地方）。
///
/// 槽位是玩家的 `ChildOf` 子实体（物理延伸）；起始装备由 [`ItemKind::for_slot`]
/// 给出——这一版还没有装备来源（掉落 / 商店），所以 PC 直接带着一身装备出生，
/// 玩家按 `T` 可以把它们卸下 / 再装上。
///
/// **为什么单独一个系统**（而不是塞进场景工厂）：物品要装在"槽位"上、槽位又挂在
/// 玩家上，是**两层子实体**，而 `bsn!` 的一层 `Children` 表达不了它——
/// 所以槽位与物品在组装之后由本系统补上。
///
/// **幂等**：只给「还没有槽位子实体」的玩家发装备。判据是玩家名下的
/// [`Children`] 里有没有 `EquipmentSlot`，不是"发过一次"这种记忆——
/// 重置（重新组装玩家）之后新玩家没有子实体，于是自动再发一遍，
/// 不需要重置逻辑记住装备这回事。
///
/// ⚠️ 判据必须是**世界里的既成事实**：这个系统每帧都跑，若只认
/// `With<InputDriven>`，它会在每一帧都发一整套（实测：护甲每帧 +2，两帧就翻了倍）。
///
/// 槽位与物品的 `Transform` 都是**局部零偏移**：单位根节点就是脚底
/// （见 [`crate::spawn::unit`]），它们挂在下面，于是**世界坐标 = PC 的世界坐标**
/// ——这正是"卸下的物品落在脚下"所需要的（见
/// [`unequip`](crate::equipment::unequip)）。
pub fn equip_starting_gear_system(
    mut commands: Commands,
    players: Query<(Entity, Option<&Children>), With<InputDriven>>,
    slots: Query<(), With<crate::equipment::EquipmentSlot>>,
) {
    for (player, children) in &players {
        let already_has_slots =
            children.is_some_and(|children| children.iter().any(|child| slots.get(child).is_ok()));
        if already_has_slots {
            continue;
        }
        for slot in SlotKind::ALL {
            let slot_entity = commands
                .spawn_scene(equipment_slot_scene(slot, player))
                .id();
            equip(&mut commands, slot_entity, ItemKind::for_slot(slot));
        }
    }
}
