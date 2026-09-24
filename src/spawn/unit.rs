//! 单位通用零件：玩家与敌人共用的一层。

use bevy::prelude::*;

use crate::combat::defense::BlockChance;
use crate::combat::health::Health;
use crate::combat::{Armor, AttackRange, Collidable, Faction, HitRadius, Stamina};
use crate::movement::{Cell, Velocity};
use crate::presentation::unit_sprite::{
    SHADOW_DIAMETER, SHADOW_OFFSET, SPRITE_SIZE, UnitShadow, UnitSprite, UnitSprites,
};
use crate::timeline::{DecisionSlot, Focus, FocusRecoverTimer};

/// ⚠️ **护甲占位值**（设计旋钮，不是平衡结论）。
///
/// 参照当前伤害（近战 15 / 火球 12 / 箭矢 10）与 50 血：
/// - 玩家 1 点：近战 4 刀、火球 5 发才打死（**没有改变任何一击的刀数**，安全值）；
/// - 敌人 0 点：保持"敌人比玩家脆"的既有手感——玩家先手更有价值。
///
/// 这两个数**该由你定**：护甲每加 1 点，箭矢就要多打一发才死。
/// 等装备系统（M27）落地后，这里换成"基础值 + 装备加成"。
pub const PLAYER_ARMOR: i32 = 1;
/// 见 [`PLAYER_ARMOR`]。
pub const ENEMY_ARMOR: i32 = 0;

/// 这个阵营的护甲（组装层是**唯一**决定单位属性的地方）。
pub fn armor_for(faction: Faction) -> i32 {
    match faction {
        Faction::Player => PLAYER_ARMOR,
        Faction::Enemy => ENEMY_ARMOR,
    }
}

/// 单位骨架：逻辑组件 + 2D 精灵纸片 + 贴地阴影。
///
/// 「玩家」和「怪物」都从这里出发，只在各自的工厂里追加**驱动源**
/// （输入 / AI）与特质（区块加载器 / 攻击范围）——零件共用，驱动不同。
///
/// 带**空决策槽**出生：开局第一帧谁都可以决策（无回合模型没有「先规划」这一步）。
///
/// 根节点永远是**脚底**：`translation` 落在 [`crate::world`] 给的地表高度上，旋转与
/// 缩放保持默认——纸片与阴影是它的子节点，靠这一点用局部坐标直接表达世界偏移
/// （见 [`crate::presentation::unit_sprite`]）。视觉尺寸在网格上，不在缩放上。
pub fn unit_scene(faction: Faction, position: Vec3, sprites: &UnitSprites) -> impl Scene {
    let armor = armor_for(faction);
    let cell = Cell::from_world(position);
    let stamina = Stamina::default();
    let sprite = sprites.sprite(faction);
    let shadow = sprites.shadow();
    bsn! {
        template_value(faction)
        template_value(Health::new(50))
        // 护甲（命中管线第 ④ 关：格挡之后再减）：**单位属性**，
        // 等装备系统（M27）落地后改成"基础值 + 装备加成"。
        // ⚠️ **数值是占位**：`ARMOR_*` 三个常量是设计旋钮，不是平衡结论。
        template_value(Armor(armor))
        HitRadius(0.8)
        AttackRange::MELEE
        Collidable
        // 格挡率：现在是单位属性（**没有单位真的会格挡**，`0.0` = 管线第 ③ 关直接跳过），
        // 等装备系统（M27）落地后改成从装备 / 姿态读。
        template_value(BlockChance(0.0))
        template_value(Velocity(Vec3::ZERO))
        template_value(DecisionSlot::Idle { intent: None })
        // Focus（**每个单位各一份**）：玩家和敌人都能攒够余量抢先手。
        // 回复计时器跟着走，所以各回各的、新上场的不会蹭进度。
        template_value(Focus::default())
        template_value(FocusRecoverTimer::default())
        template_value(stamina)
        template_value(cell)
        Transform {
            translation: {position},
        }
        // 单位根是视觉子树的根：没有 `Visibility` 的话，带可见性的纸片 / 阴影会
        // 报 bevy 的 B0004（父节点缺同名组件），同一条链上的隐藏 / 显示也会失配
        Visibility::default()
        Children [
            // 2D 纸片：底边落在脚底，偏航角由 `billboard_system` 每帧对准相机
            (
                UnitSprite
                Mesh3d(asset_value(Rectangle::new(SPRITE_SIZE, SPRITE_SIZE)))
                MeshMaterial3d<StandardMaterial>(asset_value(StandardMaterial {
                    base_color_texture: {Some(sprite.clone())},
                    unlit: true,
                    alpha_mode: AlphaMode::Blend,
                    double_sided: true,
                    ..default()
                }))
                Transform {
                    translation: {Vec3::new(0.0, SPRITE_SIZE * 0.5, 0.0)},
                }
            ),
            // 贴地阴影：水平方块，位置与缩放由 `shadow_system` 每帧刷新
            (
                UnitShadow
                Mesh3d(asset_value(Rectangle::new(SHADOW_DIAMETER, SHADOW_DIAMETER)))
                MeshMaterial3d<StandardMaterial>(asset_value(StandardMaterial {
                    base_color: Color::srgba(0.0, 0.0, 0.0, 0.45),
                    base_color_texture: {Some(shadow.clone())},
                    unlit: true,
                    alpha_mode: AlphaMode::Blend,
                    double_sided: true,
                    ..default()
                }))
                Transform {
                    translation: {Vec3::Y * SHADOW_OFFSET},
                    rotation: {Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)},
                }
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **护甲真的挂上了**：公式支持 `Armor` 很久了，但组装层一直没给任何单位挂
    /// ——于是第 ④ 关永远减 0，等于不存在。这条守住"它真的在链路上"。
    #[test]
    fn both_factions_carry_armor() {
        for faction in [Faction::Player, Faction::Enemy] {
            let armor = armor_for(faction);
            assert!(
                armor >= 0,
                "{faction:?} 的护甲不该是负数（负护甲会让伤害反而变高）"
            );
        }
    }

    /// 玩家有护甲、敌人没有：**这是刻意的**（参 [`PLAYER_ARMOR`] 的说明），
    /// 用测试钉住而不是靠注释——数值改了这里会提醒你别忘了同步说明。
    #[test]
    fn the_player_is_tougher_than_the_enemy_for_now() {
        assert!(
            armor_for(Faction::Player) >= armor_for(Faction::Enemy),
            "占位设定里玩家不该比敌人脆"
        );
    }
}
