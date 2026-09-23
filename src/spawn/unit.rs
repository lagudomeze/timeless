//! 单位通用零件：玩家与敌人共用的一层。

use bevy::prelude::*;

use crate::combat::defense::BlockChance;
use crate::combat::health::Health;
use crate::combat::{AttackRange, Collidable, Faction, HitRadius, Stamina};
use crate::movement::{Cell, Velocity};
use crate::presentation::unit_sprite::{
    SHADOW_DIAMETER, SHADOW_OFFSET, SPRITE_SIZE, UnitShadow, UnitSprite, UnitSprites,
};
use crate::timeline::DecisionSlot;

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
    let cell = Cell::from_world(position);
    let stamina = Stamina::default();
    let sprite = sprites.sprite(faction);
    let shadow = sprites.shadow();
    bsn! {
        template_value(faction)
        template_value(Health::new(50))
        HitRadius(0.8)
        AttackRange::MELEE
        Collidable
        // 格挡率：现在是单位属性（**没有单位真的会格挡**，`0.0` = 管线第 ③ 关直接跳过），
        // 等装备系统（M27）落地后改成从装备 / 姿态读。
        template_value(BlockChance(0.0))
        template_value(Velocity(Vec3::ZERO))
        template_value(DecisionSlot::Idle { intent: None })
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
