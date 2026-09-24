//! 可行走性：**纯规则**（零 Bevy、零 `world` 依赖，可脱离 App 单测）。
//!
//! 回答一件事：**这个单位能不能踏进那一格**。它刻意是个纯函数——输入是两格的
//! 地表高度，输出是能不能走。之所以不在这里查体素，是因为**地形高度是纯函数**
//! （`world::surface_height`，见 `docs/domain.md`）：不查区块也能算出任何格的高度，
//! 这条性质换来了"单位贴地不依赖流式加载状态"，不该为了可行走性破坏它。
//!
//! ## 为什么需要它
//!
//! 在此之前，移动**没有任何可行走性判定**：点哪走哪，一路直线穿过任何东西。
//! 地形起伏本身很克制（默认配置相邻格最多差 1 个体素），真正会挡住人的是
//! **玩家自己堆的墙**——而那正是"能不能过去"该有答案的地方。
//!
//! ## 门槛为什么是 1
//!
//! 一格是 2 个体素宽（`CELL_SIZE`），地形以 1 个体素为步进。**差 1 步 = 上一级台阶**
//! （走过去），**差 2 步及以上 = 一堵墙**（走不过去）。这条判据只吃高度差，
//! 因此默认地形下处处可走，只有被改高的地方才会拒绝——正是我们要的行为。

use bevy::prelude::*;

/// 能迈上去的最大高度差（体素）。
///
/// 1 = 一级台阶。见模块文档「门槛为什么是 1」。
pub const MAX_STEP_UP: i32 = 1;

/// 能不能从 `from_y` 走到 `to_y`（两者都是**地表高度**，世界体素坐标）。
///
/// 只拒绝**上不去**的：从高处往低处走永远允许（跳下去 / 走下来），
/// 否则堆一堵墙会把自己也关在里面。
pub fn can_step(from_y: i32, to_y: i32) -> bool {
    to_y - from_y <= MAX_STEP_UP
}

/// 一次移动被拒的原因（**本域自己宣布**）。
///
/// 不复用时间线的 `ActionBlocked`：那是"谁可以决策"的提示通道，
/// 而这里是**几何**——和 `world::BlockRefused` 同一种做法（谁拥有事实谁宣布）。
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveRefused {
    /// 目标格比脚下高太多：那是堵墙，不是台阶
    BlockedByTerrain,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 平地与下坡都走得通；上一级台阶可以；**两级以上就是墙**。
    #[test]
    fn a_single_step_is_walkable_and_a_wall_is_not() {
        assert!(can_step(0, 0), "平地");
        assert!(can_step(0, 1), "上一级台阶");
        assert!(!can_step(0, 2), "两级就是墙");
        assert!(can_step(2, 0), "下坡永远允许（否则会把自己关在墙里）");
        assert!(can_step(2, 1), "下一级台阶");
        assert!(
            !can_step(-1, 2),
            "高度差按数值算，与正负无关（这里是 3 级）"
        );
        assert!(can_step(-3, -2), "负高度同样只是一级台阶");
    }

    /// 门槛常量就是 1：改它等于改手感，测试钉住它，改的时候会被提醒。
    #[test]
    fn the_threshold_is_one_voxel() {
        assert_eq!(MAX_STEP_UP, 1);
        assert!(can_step(0, MAX_STEP_UP));
        assert!(!can_step(0, MAX_STEP_UP + 1));
    }
}
