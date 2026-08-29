//! # 展示层：按表现子域拆分（mod 目录）
//!
//! - `camera`：主相机与视角控制（右键拖动 / 滚轮缩放）
//! - `unit`：纸片单位与贴地阴影（Billboard / GroundShadow）+ 根节点坐标同步
//! - `hints`：动作提示（移动方向箭头）
//! - `hud`：屏幕底部 HUD
//! - `hover`：鼠标悬停格子的坐标读数（屏幕右上角）
//! - `map`：地图表现（地面 / 装饰）与网格几何常量
//!
//! 各子模块使用显式路径引用（如 `crate::display::unit::sync_transforms`），
//! 保证领域归属清晰、不产生跨子域的名字冲突。

pub mod camera;
pub mod hints;
pub mod hover;
pub mod hud;
pub mod map;
pub mod unit;
