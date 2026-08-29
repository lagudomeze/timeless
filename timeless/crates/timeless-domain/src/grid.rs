//! # 领域层：网格空间（纯 Rust，零 Bevy 依赖）
//!
//! 提供网格坐标与距离计算，供战斗裁决（射程判定）与 AI 移动使用。
//! 应用层的 `Position` 组件是本类型的 newtype 包装（见 `timeless-app/src/components.rs`）。

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
}

impl GridPos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// 切比雪夫距离（允许 8 向移动；用于射程判定：距离 1 = 相邻格）
    pub fn chebyshev(self, other: Self) -> u32 {
        self.x.abs_diff(other.x).max(self.y.abs_diff(other.y))
    }

    /// 曼哈顿距离（4 向移动用）
    pub fn manhattan(self, other: Self) -> u32 {
        self.x.abs_diff(other.x) + self.y.abs_diff(other.y)
    }

    /// 朝 `target` 移动一格（8 向，允许对角线；已在目标格则原地不动）
    pub fn step_toward(self, target: Self) -> Self {
        let dx = (target.x - self.x).signum();
        let dy = (target.y - self.y).signum();
        Self::new(self.x + dx, self.y + dy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chebyshev_distance() {
        assert_eq!(GridPos::new(0, 0).chebyshev(GridPos::new(3, 4)), 4);
        assert_eq!(GridPos::new(0, 0).chebyshev(GridPos::new(1, 1)), 1);
    }

    #[test]
    fn manhattan_distance() {
        assert_eq!(GridPos::new(0, 0).manhattan(GridPos::new(3, 4)), 7);
    }

    #[test]
    fn step_toward_moves_one_cell_diagonally() {
        let start = GridPos::new(0, 0);
        assert_eq!(start.step_toward(GridPos::new(3, 2)), GridPos::new(1, 1));
    }

    #[test]
    fn step_toward_does_not_move_when_at_target() {
        let p = GridPos::new(2, 2);
        assert_eq!(p.step_toward(p), p);
    }
}
