//! 威胁覆盖格的纯函数（零 ECS，可单独单测）。

use crate::movement::Cell;

/// 近战扇形覆盖的格：**正前方一格 + 左右各一格**。
///
/// 真正的扇形是几何（`MeleeShape` 用真实距离与夹角判定），这里只是**决策层**的
/// 近似：反应系统要回答"站在这几格里会不会挨这一刀"，三格的近似足够表达
/// 「正面有威胁、侧后方安全」，而且和 AI 走格的世界完全对齐。
pub fn melee_arc_cells(origin: Cell, target: Cell) -> Vec<Cell> {
    let (dx, dz) = (target.x - origin.x, target.z - origin.z);
    // 用主导分量吸附成一个正交方向（与 `step_from_axis` 同一约定）
    let step = if dx.abs() > dz.abs() {
        (dx.signum(), 0)
    } else if dz != 0 {
        (0, dz.signum())
    } else {
        (0, 0)
    };
    if step == (0, 0) {
        return vec![target]; // 贴身对同格：只有脚下那一格
    }
    // 与挥击方向垂直的轴：刀锋扫过的两侧
    let side = if step.0 != 0 { (0, 1) } else { (1, 0) };
    vec![
        target,
        Cell::new(target.x - side.0, target.z - side.1),
        Cell::new(target.x + side.0, target.z + side.1),
    ]
}

/// 直线飞行覆盖的格（含起点与终点）。
///
/// 用整数 DDA 沿较长的那条轴走：每一步落进哪一格就收哪一格，重复的合并掉。
/// 火球用它声明"我这一路会经过哪些格"，玩家的反应因此可以提前到飞行途中。
pub fn trajectory_cells(from: Cell, to: Cell) -> Vec<Cell> {
    let steps = (to.x - from.x).abs().max((to.z - from.z).abs());
    if steps == 0 {
        return vec![from];
    }
    let mut cells = Vec::with_capacity(steps as usize + 1);
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let x = from.x + ((to.x - from.x) as f32 * t).round() as i32;
        let z = from.z + ((to.z - from.z) as f32 * t).round() as i32;
        let cell = Cell::new(x, z);
        if cells.last() != Some(&cell) {
            cells.push(cell);
        }
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn melee_arc_covers_the_front_cell_and_both_flanks() {
        let arc = melee_arc_cells(Cell::new(0, 0), Cell::new(1, 0));
        assert_eq!(
            arc,
            vec![Cell::new(1, 0), Cell::new(1, -1), Cell::new(1, 1)],
            "朝 +X 横扫：正前方一格 + 左右各一格"
        );

        let arc = melee_arc_cells(Cell::new(0, 0), Cell::new(0, 2));
        assert_eq!(
            arc,
            vec![Cell::new(0, 2), Cell::new(-1, 2), Cell::new(1, 2)],
            "朝 +Z 横扫：侧向换到 X 轴"
        );
    }

    #[test]
    fn melee_arc_of_a_same_cell_stack_is_just_that_cell() {
        assert_eq!(
            melee_arc_cells(Cell::new(2, 2), Cell::new(2, 2)),
            vec![Cell::new(2, 2)]
        );
    }

    #[test]
    fn trajectory_is_a_straight_line_of_cells() {
        assert_eq!(
            trajectory_cells(Cell::new(0, 0), Cell::new(4, 0)),
            vec![
                Cell::new(0, 0),
                Cell::new(1, 0),
                Cell::new(2, 0),
                Cell::new(3, 0),
                Cell::new(4, 0),
            ]
        );
        assert_eq!(
            trajectory_cells(Cell::new(0, 0), Cell::new(0, 0)),
            vec![Cell::new(0, 0)],
            "零距离只覆盖脚下那一格"
        );
    }
}
